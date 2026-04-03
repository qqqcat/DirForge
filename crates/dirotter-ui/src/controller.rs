use crate::DeleteRequestScope;
use dirotter_actions::{execute_plan, DeletionPlan, ExecutionMode, ExecutionReport};
use eframe::egui;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub(crate) struct DeleteSession {
    pub(crate) relay: Arc<Mutex<DeleteRelayState>>,
}

pub(crate) struct MemoryReleaseSession {
    pub(crate) relay: Arc<Mutex<MemoryReleaseRelayState>>,
}

pub(crate) struct DeleteRelayState {
    pub(crate) started_at: Instant,
    pub(crate) label: String,
    pub(crate) target_count: usize,
    pub(crate) mode: ExecutionMode,
    pub(crate) finished: Option<DeleteFinishedPayload>,
}

#[derive(Clone)]
pub(crate) struct MemoryReleaseRelayState {
    pub(crate) finished: Option<
        Result<dirotter_platform::SystemMemoryReleaseReport, dirotter_platform::PlatformError>,
    >,
}

pub(crate) struct DeleteFinishedPayload {
    pub(crate) request: DeleteRequestScope,
    pub(crate) report: ExecutionReport,
}

pub(crate) struct QueuedDeleteRequest {
    pub(crate) request: DeleteRequestScope,
    pub(crate) mode: ExecutionMode,
}

impl DeleteRelayState {
    pub(crate) fn new(request: &DeleteRequestScope, mode: ExecutionMode) -> Self {
        Self {
            started_at: Instant::now(),
            label: request.label.clone(),
            target_count: request.targets.len(),
            mode,
            finished: None,
        }
    }
}

impl DeleteSession {
    pub(crate) fn snapshot(&self) -> DeleteRelayState {
        let relay = self.relay.lock().expect("delete relay lock");
        DeleteRelayState {
            started_at: relay.started_at,
            label: relay.label.clone(),
            target_count: relay.target_count,
            mode: relay.mode,
            finished: None,
        }
    }
}

pub(crate) fn start_memory_release_session(ctx: egui::Context) -> MemoryReleaseSession {
    let relay = Arc::new(Mutex::new(MemoryReleaseRelayState { finished: None }));
    let relay_state = Arc::clone(&relay);
    std::thread::spawn(move || {
        let result = dirotter_platform::release_system_memory();
        let mut state = relay_state.lock().expect("memory release relay lock");
        state.finished = Some(result);
        drop(state);
        ctx.request_repaint();
    });
    MemoryReleaseSession { relay }
}

pub(crate) fn take_finished_memory_release(
    session: &MemoryReleaseSession,
) -> Option<Result<dirotter_platform::SystemMemoryReleaseReport, dirotter_platform::PlatformError>>
{
    let mut relay = session.relay.lock().expect("memory release relay lock");
    relay.finished.take()
}

pub(crate) fn start_delete_session(
    ctx: egui::Context,
    request: DeleteRequestScope,
    plan: DeletionPlan,
    mode: ExecutionMode,
) -> DeleteSession {
    let relay = Arc::new(Mutex::new(DeleteRelayState::new(&request, mode)));
    let relay_state = Arc::clone(&relay);
    std::thread::spawn(move || {
        let report = execute_plan(&plan, mode);
        let mut state = relay_state.lock().expect("delete relay lock");
        state.finished = Some(DeleteFinishedPayload { request, report });
        drop(state);
        ctx.request_repaint();
    });
    DeleteSession { relay }
}

pub(crate) fn take_finished_delete(session: &DeleteSession) -> Option<DeleteFinishedPayload> {
    let mut relay = session.relay.lock().expect("delete relay lock");
    relay.finished.take()
}
