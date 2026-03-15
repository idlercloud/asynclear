use core::{
    ops::ControlFlow,
    pin::Pin,
    ptr::NonNull,
    sync::atomic::Ordering,
    task::{Context, Poll},
};

use defines::error::errno;
use executor::time;
use kernel_tracer::Instrument;
use libkernel::{
    drivers::{qemu_plic::Plic, qemu_uart, InterruptSource},
    extern_symbols, hart, memory, process,
    thread::{Thread, ThreadStatus},
    trap::{self, TrapContext},
};
use riscv::{
    interrupt::{Exception, Interrupt, Trap},
    register::{
        scause, sepc, sie, sstatus, stval,
        stvec::{self, TrapMode},
    },
};
use triomphe::Arc;

use crate::syscall;

pub fn spawn_user_thread(thread: Arc<Thread>) {
    let (runnable, task) = executor::spawn_with(
        UserThreadWrapperFuture::new(Arc::clone(&thread), user_thread_loop()),
        move || thread.set_status(ThreadStatus::Ready),
    );
    runnable.schedule();
    task.detach();
}

pub async fn user_thread_loop() {
    loop {
        // 返回用户态
        // 注意切换了控制流，但是之后回到内核态还是在这里
        trace!("enter user mode");
        trap_return(hart::local_hart().curr_trap_context());
        trace!("enter kernel mode");

        // 在内核态处理 trap。注意这里也可能切换控制流，让出 Hart 给其他线程
        let next_op = user_trap_handler()
            .instrument({
                let process_name = { hart::local_hart().curr_process().name() };
                info_span!("process", name = process_name)
            })
            .await;

        if next_op.is_break() || hart::local_hart().curr_process().is_exited() {
            break;
        }
    }
}

/// `UserThreadWrapperFuture` 用来处理用户线程获取控制权以及让出控制权时的上下文切换。如页表切换等
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project::pin_project]
struct UserThreadWrapperFuture<UserThreadFuture> {
    #[pin]
    future: UserThreadFuture,
    thread: Arc<Thread>,
}

impl<UserThreadFuture> UserThreadWrapperFuture<UserThreadFuture> {
    #[inline]
    fn new(thread: Arc<Thread>, future: UserThreadFuture) -> Self {
        Self { thread, future }
    }
}

impl<UserThreadFuture: Future<Output = ()>> Future for UserThreadWrapperFuture<UserThreadFuture> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        hart::local_hart().replace_thread(Some(Arc::clone(&self.thread)));
        let process = &self.thread.process;
        process.lock_inner_with(|inner| inner.memory_space.activate());
        let pid = process.pid();
        let tid = self.thread.tid();
        let _enter = info_span!("task", pid = pid, tid = tid).entered();
        let trap_context = unsafe { &mut self.thread.get_owned().as_mut().trap_context };
        if trap_context.user_float_ctx.valid {
            trap_context.user_float_ctx.restore();
        }
        trace!("User task running");
        let prev_status = self.thread.status.swap(ThreadStatus::Running, Ordering::SeqCst);
        if prev_status != ThreadStatus::Ready {
            panic!("Run unready({prev_status:?}) task")
        }

        let project = self.project();
        let ret = project.future.poll(cx);

        if ret.is_ready() {
            project.thread.exit_thread();
        } else {
            if project.thread.status.load(Ordering::SeqCst) != ThreadStatus::Ready {
                project.thread.set_status(ThreadStatus::Blocking);
            }
            let trap_context = unsafe { &mut project.thread.get_owned().as_mut().trap_context };
            if trap_context.fs() == sstatus::FS::Dirty {
                // 进入信号处理前需要保存当前线程的浮点数上下文以便信号处理完成后恢复
                trap_context.user_float_ctx.save();
                trap_context.user_float_ctx.valid = true;
                trap_context.set_fs(sstatus::FS::Clean);
            }
        }

        // NOTE: 一定要切换页表。否则进程页表被回收立刻导致内核异常
        // 但可以不刷新 tlb。因为内核中只会用到共享的、永远映射的内核高地址空间
        unsafe {
            memory::KERNEL_SPACE.activate_no_tlb();
        }
        trace!("User task deactivate");
        hart::local_hart().replace_thread(None);

        ret
    }
}

/// 在某些情况下，如调用了 `sys_exit`，会返回 `ControlFlow::Break` 以通知结束用户线程循环
pub async fn user_trap_handler() -> ControlFlow<(), ()> {
    set_kernel_trap_entry();

    // NOTE: `scause` 和 `stval` 一定要在开中断前读，因为它们会被中断覆盖
    let scause = scause::read();
    let stval = stval::read();

    unsafe {
        sstatus::set_sie();
    }

    match scause.cause() {
        Trap::Exception(e) if e == Exception::UserEnvCall as usize => {
            let (syscall_id, syscall_args) = {
                let trap_context = unsafe { hart::local_hart().curr_trap_context().as_mut() };
                // TODO: syscall 的返回位置是下一条指令，不过一定是 +4 吗？
                trap_context.sepc += 4;
                let user_regs = &mut trap_context.user_regs;
                let syscall_id = user_regs[16];
                let syscall_args = [
                    user_regs[9],
                    user_regs[10],
                    user_regs[11],
                    user_regs[12],
                    user_regs[13],
                    user_regs[14],
                ];
                (syscall_id, syscall_args)
            };
            let result = syscall::syscall(syscall_id, syscall_args)
                .instrument(info_span!("syscall", name = defines::syscall::name(syscall_id)))
                .await;

            // 线程应当退出
            if result == errno::BREAK.as_isize() {
                ControlFlow::Break(())
            } else {
                unsafe {
                    *hart::local_hart().curr_trap_context().as_mut().a0_mut() = result as usize;
                }
                ControlFlow::Continue(())
            }
        }

        Trap::Exception(e)
            if e == Exception::StoreFault as usize
                || e == Exception::StorePageFault as usize
                || e == Exception::InstructionPageFault as usize
                || e == Exception::LoadPageFault as usize =>
        {
            let _enter = info_span!("pagefault").entered();
            let thread = hart::local_hart().curr_thread();

            let ok = thread.process.lock_inner_with(|inner| {
                inner
                    .memory_space
                    .handle_memory_exception(stval, e == Exception::StoreFault as usize)
            });

            if ok {
                ControlFlow::Continue(())
            } else {
                let trap_context = unsafe { &mut thread.get_owned().as_mut().trap_context };
                info!("regs: {:x?}", trap_context.user_regs);
                error!(
                    "{:?} in application, bad addr = {:#x}, bad inst pc = {:#x}, core dumped.",
                    scause.cause(),
                    stval,
                    trap_context.sepc,
                );
                process::exit_process(&thread.process, -2);
                ControlFlow::Break(())
            }
        }
        Trap::Exception(e) if e == Exception::IllegalInstruction as usize => {
            let thread = hart::local_hart().curr_thread();
            let trap_context = unsafe { &mut thread.get_owned().as_mut().trap_context };
            info!("regs: {:x?}", trap_context.user_regs);
            error!(
                "IllegalInstruction(pc={:#x}) in application, core dumped.",
                trap_context.sepc,
            );
            process::exit_process(&thread.process, -3);
            ControlFlow::Break(())
        }
        Trap::Interrupt(e) if e == Interrupt::SupervisorTimer as usize => {
            {
                let _enter = debug_span!("timer_irq").entered();
                time::check_timer();
                riscv_time::set_next_trigger();
            }
            executor::yield_now().await;
            ControlFlow::Continue(())
        }
        Trap::Interrupt(e) if e == Interrupt::SupervisorExternal as usize => {
            let _enter = debug_span!("external_irq").entered();
            interrupt_handler();
            ControlFlow::Continue(())
        }
        _ => {
            panic!("Unsupported trap {:?}, stval = {:#x}!", scause.cause(), stval,);
        }
    }
}

pub fn init_trap() {
    set_kernel_trap_entry();
    unsafe {
        sie::set_sext();
        sie::set_stimer();
        sstatus::set_sie();
    }
    riscv_time::set_next_trigger();
}

fn set_user_trap_entry() {
    unsafe {
        stvec::write(extern_symbols::__trap_from_user as *const () as usize, TrapMode::Direct);
    }
}

fn set_kernel_trap_entry() {
    unsafe {
        stvec::write(
            extern_symbols::__trap_from_kernel as *const () as usize,
            TrapMode::Direct,
        );
    }
}

/// Kernel trap handler
#[unsafe(no_mangle)]
pub extern "C" fn kernel_trap_handler() {
    match scause::read().cause() {
        Trap::Interrupt(i) if i == Interrupt::SupervisorTimer as usize => {
            let _enter = debug_span!("timer_irq").entered();
            // TODO: 想办法通知线程让出 hart
            time::check_timer();
            riscv_time::set_next_trigger();
        }
        Trap::Interrupt(i) if i == Interrupt::SupervisorExternal as usize => {
            let _enter = debug_span!("external_irq").entered();
            interrupt_handler();
        }
        other => {
            panic!(
                "Trap from kernel! Cause = {:?}, bad addr = {:#x}, bad instruction = {:#x}",
                other,
                stval::read(),
                sepc::read(),
            );
        }
    }
}

/// 从用户任务的内核态返回到用户态。
///
/// 注意：会切换控制流和栈
pub fn trap_return(trap_context: NonNull<TrapContext>) {
    trap::check_signal(&hart::local_hart().curr_thread());

    // 因为 trap entry 要切换为用户的，在回到用户态之前不能触发中断
    unsafe {
        sstatus::clear_sie();
    }
    set_user_trap_entry();

    unsafe {
        // 对内核来说，调用 __return_to_user 返回内核态就好像一次函数调用
        // 因此编译器会将 Caller Saved 的寄存器保存下来
        // 但是 Called Saved 的寄存器很快会被覆盖，因此需要在 TrapContext 上保存下来
        extern_symbols::__return_to_user(trap_context);
    }
}

pub fn interrupt_handler() {
    let plic = unsafe { &*Plic::mmio() };
    let hart_id = hart::local_hart().hart_id();
    let context_id = hart_id * 2;
    let interrupt_id = plic.claim(context_id);
    // 为 0 应该说明是多个核争抢同一个中断，然后没抢到？
    if interrupt_id == 0 {
        return;
    }
    let Some(interrupt_source) = InterruptSource::from_id(interrupt_id) else {
        panic!("Unknown interrupt {interrupt_id}");
    };
    match interrupt_source {
        InterruptSource::Uart0 => qemu_uart::UART0.handle_irq(),
        InterruptSource::VirtIO => todo!("[mid] virtio interrupt handler"),
    }
    plic.complete(context_id, interrupt_id);
}
