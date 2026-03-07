use std::{
    cell::RefCell,
    mem,
    time::{Duration, Instant},
};

#[derive(Default)]
struct TimingState {
    session_depth: usize,
    stack: Vec<TimerNode>,
    roots: Vec<TimerNode>,
}

struct TimerNode {
    label: String,
    start: Instant,
    elapsed: Duration,
    children: Vec<TimerNode>,
}

impl TimerNode {
    fn new(label: String) -> Self {
        Self {
            label,
            start: Instant::now(),
            elapsed: Duration::ZERO,
            children: Vec::new(),
        }
    }

    fn stop(mut self) -> Self {
        self.elapsed = self.start.elapsed();
        self
    }

    fn self_elapsed(&self) -> Duration {
        let children_total = self
            .children
            .iter()
            .map(|child| child.elapsed)
            .fold(Duration::ZERO, |acc, d| acc + d);
        self.elapsed.saturating_sub(children_total)
    }
}

thread_local! {
    static TIMING_STATE: RefCell<TimingState> = RefCell::new(TimingState::default());
}

pub struct TimerSession;

impl TimerSession {
    pub fn start() -> Self {
        TIMING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.session_depth += 1;
            if state.session_depth == 1 {
                state.stack.clear();
                state.roots.clear();
            }
        });
        Self
    }
}

impl Drop for TimerSession {
    fn drop(&mut self) {
        let roots = TIMING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            if state.session_depth == 0 {
                return None;
            }
            state.session_depth -= 1;
            if state.session_depth != 0 {
                return None;
            }
            Some(mem::take(&mut state.roots))
        });

        if let Some(roots) = roots {
            render_report(&roots);
        }
    }
}

pub struct ScopedTimer {
    enabled: bool,
}

impl ScopedTimer {
    pub fn start(label: impl Into<String>) -> Self {
        let label = label.into();
        let enabled = TIMING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            if state.session_depth == 0 {
                return false;
            }
            state.stack.push(TimerNode::new(label));
            true
        });
        Self { enabled }
    }
}

impl Drop for ScopedTimer {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }
        TIMING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let Some(node) = state.stack.pop() else {
                return;
            };
            let node = node.stop();
            if let Some(parent) = state.stack.last_mut() {
                parent.children.push(node);
            } else {
                state.roots.push(node);
            }
        });
    }
}

fn render_report(roots: &[TimerNode]) {
    if roots.is_empty() {
        return;
    }
    println!("[timer] timing tree:");
    for (idx, root) in roots.iter().enumerate() {
        let is_last = idx + 1 == roots.len();
        render_node(root, "", is_last);
    }
}

fn render_node(node: &TimerNode, prefix: &str, is_last: bool) {
    let branch = if is_last { "└─ " } else { "├─ " };
    if node.children.is_empty() {
        println!(
            "[timer] {}{}{} ({})",
            prefix,
            branch,
            node.label,
            format_duration(node.elapsed)
        );
    } else {
        println!(
            "[timer] {}{}{} (total {}, self {})",
            prefix,
            branch,
            node.label,
            format_duration(node.elapsed),
            format_duration(node.self_elapsed())
        );
    }

    let child_prefix = if is_last {
        format!("{prefix}   ")
    } else {
        format!("{prefix}│  ")
    };
    for (idx, child) in node.children.iter().enumerate() {
        let child_is_last = idx + 1 == node.children.len();
        render_node(child, &child_prefix, child_is_last);
    }
}

fn format_duration(duration: Duration) -> String {
    format!("{:.3}s", duration.as_secs_f64())
}
