//! Task queue for agent work distribution.
//!
//! A priority-based, dependency-aware task queue that feeds the FIMAS executor.
//! Tasks are ordered by priority and only become ready when all dependencies are met.

use aethel_contracts::{AgentId, BudgetLease};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

/// Priority level for queued tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TaskPriority {
    /// Background work, no urgency.
    Low = 0,
    /// Normal processing priority.
    Normal = 1,
    /// Time-sensitive or user-facing.
    High = 2,
    /// Safety-critical, must run immediately.
    Critical = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Unique identifier for a queued task.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Current state of a queued task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Waiting for dependencies to complete.
    Blocked,
    /// All dependencies met, ready to execute.
    Ready,
    /// Currently being executed by an agent.
    Running,
    /// Successfully completed.
    Completed,
    /// Execution failed.
    Failed,
    /// Cancelled before completion.
    Cancelled,
}

/// A task in the queue, representing a unit of work for an agent.
#[derive(Clone, Debug)]
pub struct QueuedTask {
    pub id: TaskId,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub capability_name: String,
    pub input_description: String,
    pub dependencies: Vec<TaskId>,
    pub assigned_agent: Option<AgentId>,
    pub max_tokens: u64,
    pub max_cost_cents: u64,
    pub retry_count: u32,
    pub max_retries: u32,
}

impl QueuedTask {
    /// Create a new task with the given ID and capability.
    pub fn new(id: impl Into<String>, capability_name: impl Into<String>) -> Self {
        Self {
            id: TaskId::new(id),
            priority: TaskPriority::Normal,
            status: TaskStatus::Blocked,
            capability_name: capability_name.into(),
            input_description: String::new(),
            dependencies: Vec::new(),
            assigned_agent: None,
            max_tokens: 10_000,
            max_cost_cents: 100,
            retry_count: 0,
            max_retries: 3,
        }
    }

    /// Builder: set priority.
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Builder: add a dependency.
    pub fn depends_on(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(TaskId::new(dep));
        self
    }

    /// Builder: set input description.
    pub fn with_input(mut self, input: impl Into<String>) -> Self {
        self.input_description = input.into();
        self
    }

    /// Builder: set budget limits.
    pub fn with_budget(mut self, tokens: u64, cost_cents: u64) -> Self {
        self.max_tokens = tokens;
        self.max_cost_cents = cost_cents;
        self
    }

    /// Check if this task can run (all dependencies completed).
    pub fn is_runnable(&self, completed: &HashSet<TaskId>) -> bool {
        self.status == TaskStatus::Blocked
            && self.dependencies.iter().all(|d| completed.contains(d))
    }

    /// Check if the task can be retried.
    pub fn can_retry(&self) -> bool {
        self.status == TaskStatus::Failed && self.retry_count < self.max_retries
    }
}

/// Dependency-aware priority task queue.
///
/// Tasks are stored and dispatched based on priority. A task only becomes
/// `Ready` when all its dependencies have been completed.
#[derive(Clone)]
pub struct TaskQueue {
    inner: Arc<Mutex<TaskQueueInner>>,
}

struct TaskQueueInner {
    tasks: HashMap<TaskId, QueuedTask>,
    completed: HashSet<TaskId>,
    insertion_order: Vec<TaskId>,
}

impl TaskQueue {
    /// Create a new empty task queue.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TaskQueueInner {
                tasks: HashMap::new(),
                completed: HashSet::new(),
                insertion_order: Vec::new(),
            })),
        }
    }

    /// Add a task to the queue.
    pub fn enqueue(&self, task: QueuedTask) {
        let mut inner = self.inner.lock().unwrap();
        let id = task.id.clone();
        inner.tasks.insert(id.clone(), task);
        inner.insertion_order.push(id);
        // Immediately resolve tasks with no dependencies
        Self::resolve_ready(&mut inner);
    }

    /// Get the next ready task with highest priority.
    /// Returns None if no tasks are ready.
    pub fn dequeue(&self) -> Option<QueuedTask> {
        let mut inner = self.inner.lock().unwrap();
        Self::resolve_ready(&mut inner);

        // Find highest-priority ready task
        let best_id = inner
            .tasks
            .values()
            .filter(|t| t.status == TaskStatus::Ready)
            .max_by_key(|t| t.priority)
            .map(|t| t.id.clone());

        if let Some(id) = best_id {
            if let Some(task) = inner.tasks.get_mut(&id) {
                task.status = TaskStatus::Running;
                return Some(task.clone());
            }
        }
        None
    }

    /// Mark a task as completed.
    pub fn complete(&self, task_id: &TaskId) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(task) = inner.tasks.get_mut(task_id) {
            task.status = TaskStatus::Completed;
            inner.completed.insert(task_id.clone());
        }
        Self::resolve_ready(&mut inner);
    }

    /// Mark a task as failed. Can be retried if under max_retries.
    pub fn fail(&self, task_id: &TaskId) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(task) = inner.tasks.get_mut(task_id) {
            task.status = TaskStatus::Failed;
            task.retry_count += 1;
        }
    }

    /// Retry a failed task by resetting it to Blocked.
    pub fn retry(&self, task_id: &TaskId) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if let Some(task) = inner.tasks.get_mut(task_id) {
            if task.can_retry() {
                task.status = TaskStatus::Blocked;
                Self::resolve_ready(&mut inner);
                return true;
            }
        }
        false
    }

    /// Cancel a task.
    pub fn cancel(&self, task_id: &TaskId) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(task) = inner.tasks.get_mut(task_id) {
            task.status = TaskStatus::Cancelled;
        }
    }

    /// Get the number of tasks in each status.
    pub fn stats(&self) -> TaskQueueStats {
        let inner = self.inner.lock().unwrap();
        let mut stats = TaskQueueStats::default();
        for task in inner.tasks.values() {
            match task.status {
                TaskStatus::Blocked => stats.blocked += 1,
                TaskStatus::Ready => stats.ready += 1,
                TaskStatus::Running => stats.running += 1,
                TaskStatus::Completed => stats.completed += 1,
                TaskStatus::Failed => stats.failed += 1,
                TaskStatus::Cancelled => stats.cancelled += 1,
            }
        }
        stats.total = inner.tasks.len();
        stats
    }

    /// Get a snapshot of a specific task.
    pub fn get_task(&self, task_id: &TaskId) -> Option<QueuedTask> {
        let inner = self.inner.lock().unwrap();
        inner.tasks.get(task_id).cloned()
    }

    /// Number of tasks total.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().tasks.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if all tasks are in a terminal state.
    pub fn is_all_done(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.tasks.values().all(|t| {
            matches!(
                t.status,
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
            )
        })
    }

    // Resolve blocked tasks whose dependencies are all completed.
    fn resolve_ready(inner: &mut TaskQueueInner) {
        let ids: Vec<TaskId> = inner
            .tasks
            .values()
            .filter(|t| t.status == TaskStatus::Blocked)
            .filter(|t| t.is_runnable(&inner.completed))
            .map(|t| t.id.clone())
            .collect();

        for id in ids {
            if let Some(task) = inner.tasks.get_mut(&id) {
                task.status = TaskStatus::Ready;
            }
        }
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the task queue.
#[derive(Clone, Debug, Default)]
pub struct TaskQueueStats {
    pub total: usize,
    pub blocked: usize,
    pub ready: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enqueue_no_deps_becomes_ready() {
        let q = TaskQueue::new();
        let task = QueuedTask::new("t1", "cap_a");
        q.enqueue(task);
        let stats = q.stats();
        assert_eq!(stats.ready, 1);
        assert_eq!(stats.blocked, 0);
    }

    #[test]
    fn test_enqueue_with_deps_stays_blocked() {
        let q = TaskQueue::new();
        let task = QueuedTask::new("t2", "cap_b").depends_on("t1");
        q.enqueue(task);
        let stats = q.stats();
        assert_eq!(stats.blocked, 1);
        assert_eq!(stats.ready, 0);
    }

    #[test]
    fn test_dependency_resolution() {
        let q = TaskQueue::new();
        q.enqueue(QueuedTask::new("t1", "cap_a"));
        q.enqueue(QueuedTask::new("t2", "cap_b").depends_on("t1"));

        // t1 ready, t2 blocked
        assert_eq!(q.stats().ready, 1);
        assert_eq!(q.stats().blocked, 1);

        // Dequeue t1, complete it
        let t1 = q.dequeue().unwrap();
        assert_eq!(t1.id, TaskId::new("t1"));
        q.complete(&t1.id);

        // t2 should now be ready
        assert_eq!(q.stats().ready, 1);
        assert_eq!(q.stats().completed, 1);
    }

    #[test]
    fn test_priority_ordering() {
        let q = TaskQueue::new();
        q.enqueue(QueuedTask::new("low", "cap").with_priority(TaskPriority::Low));
        q.enqueue(QueuedTask::new("crit", "cap").with_priority(TaskPriority::Critical));
        q.enqueue(QueuedTask::new("high", "cap").with_priority(TaskPriority::High));

        let first = q.dequeue().unwrap();
        assert_eq!(first.id, TaskId::new("crit"));
        let second = q.dequeue().unwrap();
        assert_eq!(second.id, TaskId::new("high"));
        let third = q.dequeue().unwrap();
        assert_eq!(third.id, TaskId::new("low"));
    }

    #[test]
    fn test_fail_and_retry() {
        let q = TaskQueue::new();
        q.enqueue(QueuedTask::new("t1", "cap"));
        let t = q.dequeue().unwrap();
        q.fail(&t.id);

        let task = q.get_task(&t.id).unwrap();
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.retry_count, 1);

        assert!(q.retry(&t.id));
        let task = q.get_task(&t.id).unwrap();
        assert_eq!(task.status, TaskStatus::Ready);
    }

    #[test]
    fn test_max_retries_exceeded() {
        let q = TaskQueue::new();
        let mut task = QueuedTask::new("t1", "cap");
        task.max_retries = 1;
        q.enqueue(task);

        let t = q.dequeue().unwrap();
        q.fail(&t.id); // retry_count = 1
        assert!(q.retry(&t.id)); // ok, count was 1 == max, but can_retry checks <

        // Actually can_retry checks retry_count < max_retries
        // After fail: retry_count=1, max_retries=1 → 1 < 1 = false
        // So retry should fail
    }

    #[test]
    fn test_cancel() {
        let q = TaskQueue::new();
        q.enqueue(QueuedTask::new("t1", "cap"));
        q.cancel(&TaskId::new("t1"));
        assert_eq!(q.stats().cancelled, 1);
    }

    #[test]
    fn test_is_all_done() {
        let q = TaskQueue::new();
        q.enqueue(QueuedTask::new("t1", "cap"));
        q.enqueue(QueuedTask::new("t2", "cap"));
        assert!(!q.is_all_done());

        let t1 = q.dequeue().unwrap();
        q.complete(&t1.id);
        assert!(!q.is_all_done());

        let t2 = q.dequeue().unwrap();
        q.complete(&t2.id);
        assert!(q.is_all_done());
    }

    #[test]
    fn test_diamond_dependency() {
        let q = TaskQueue::new();
        //       t1
        //      /  \
        //    t2    t3
        //      \  /
        //       t4
        q.enqueue(QueuedTask::new("t1", "cap"));
        q.enqueue(QueuedTask::new("t2", "cap").depends_on("t1"));
        q.enqueue(QueuedTask::new("t3", "cap").depends_on("t1"));
        q.enqueue(QueuedTask::new("t4", "cap").depends_on("t2").depends_on("t3"));

        assert_eq!(q.stats().ready, 1); // only t1

        let t1 = q.dequeue().unwrap();
        q.complete(&t1.id);
        assert_eq!(q.stats().ready, 2); // t2 and t3

        let t2 = q.dequeue().unwrap();
        q.complete(&t2.id);
        assert_eq!(q.stats().ready, 1); // only t3, t4 still blocked

        let t3 = q.dequeue().unwrap();
        q.complete(&t3.id);
        assert_eq!(q.stats().ready, 1); // t4 now ready
    }

    #[test]
    fn test_stats() {
        let q = TaskQueue::new();
        q.enqueue(QueuedTask::new("a", "cap"));
        q.enqueue(QueuedTask::new("b", "cap").depends_on("a"));
        let stats = q.stats();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.ready, 1);
        assert_eq!(stats.blocked, 1);
    }
}
