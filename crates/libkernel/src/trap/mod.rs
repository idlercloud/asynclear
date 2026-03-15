mod context;

pub use context::TrapContext;
use defines::{error::errno, signal::SignalActionFlags};
use riscv::register::sstatus::FS;

use crate::{
    memory::UserCheck,
    process::exit_process,
    signal::{DefaultHandler, KSignalActionExt, KSignalSet, SignalContext, SIG_DFL, SIG_ERR, SIG_IGN},
    thread::Thread,
};

/// 如果进程因为信号被终止了，则返回 true
pub fn check_signal(thread: &Thread) -> bool {
    let first_pending = {
        let mut inner = thread.lock_inner();
        let pendings = inner.pending_signal.intersection(!inner.signal_mask);
        let Some(first_pending) = pendings.first_pending() else {
            return false;
        };
        inner.pending_signal.remove(KSignalSet::from(first_pending));
        first_pending
    };

    debug!("handle signal {first_pending:?}");
    let action = thread
        .process
        .lock_inner_with(|inner| inner.signal_handlers.action(first_pending).clone());
    trace!(
        "handler: {:#x}, mask: {:?}, flags: {:?}, restorer: {:#x}",
        action.handler,
        action.kmask(),
        action.flags,
        action.restorer
    );

    let handler = match action.handler {
        SIG_ERR => todo!("[low] maybe there is no `SIG_ERR`"),
        SIG_DFL => match DefaultHandler::new(first_pending) {
            DefaultHandler::Terminate | DefaultHandler::CoreDump => {
                exit_process(&thread.process, (first_pending as i8).wrapping_add_unsigned(128));
                // TODO:[low] 要处理 CoreDump
                return true;
            }
            DefaultHandler::Ignore => return false,
            DefaultHandler::Stop | DefaultHandler::Continue => {
                // 被信号 stop 或者 continue 都要通知 `sys_wait4()`
                todo!("[low] default handler Stop and Continue")
            }
        },
        SIG_IGN => return false,
        handler => handler,
    };

    let old_mask = thread.lock_inner_with(|inner| {
        let old_mask = inner.signal_mask;
        inner.signal_mask.insert(action.kmask());
        if !action.flags.contains(SignalActionFlags::SA_NODEFER) {
            inner.signal_mask.set(KSignalSet::from(first_pending), true);
        }
        old_mask
    });
    let trap_context = unsafe { &mut thread.get_owned().as_mut().trap_context };

    let mut signal_context = SignalContext {
        old_mask,
        old_trap_context: trap_context.clone(),
    };

    // 任何信号处理都可以视作一个新的任务，因此需要单独记录浮点数的使用
    trap_context.user_float_ctx.valid = false;
    if trap_context.fs() == FS::Dirty {
        debug!("save float ctx");
        // 进入信号处理前需要保存当前线程的浮点数上下文以便信号处理完成后恢复
        signal_context.old_trap_context.user_float_ctx.save();
        signal_context.old_trap_context.user_float_ctx.valid = true;
        trap_context.set_fs(FS::Clean);
    }

    trap_context.sepc = handler;
    *trap_context.sp_mut() = trap_context.sp() - core::mem::size_of::<SignalContext>();
    *trap_context.ra_mut() = action.restorer;
    *trap_context.a0_mut() = first_pending.to_user() as usize;

    let sp = signal_context.old_trap_context.sp() - core::mem::size_of::<SignalContext>();
    let user_ptr = (|| unsafe {
        UserCheck::new(sp as *mut SignalContext)
            .ok_or(errno::EINVAL)?
            .check_ptr_mut()
    })();
    if let Ok(user_ptr) = user_ptr {
        user_ptr.write(signal_context);
        false
    } else {
        exit_process(&thread.process, (first_pending as i8).wrapping_add_unsigned(128));
        true
    }
}
