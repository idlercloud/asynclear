// use super::KERNEL_TRACER_IMPL;

// FIXME: 修复 profiling 并去除 `#[allow(unused)]`
#[allow(unused)]
pub enum ProfilingEvent {
    NewSpan { id: u32, name: &'static str },
    Enter { hart_id: u32, id: u32, instant: u64 },
    Exit { id: u32, instant: u64 },
}

pub fn report_profiling() {
    // let mut fs = LOG_FS.lock();
    // writeln!(fs, "<Profiling Report>").unwrap();
    // for event in &*KERNEL_TRACER_IMPL.get().unwrap().events.lock() {
    //     match event {
    //         ProfilingEvent::NewSpan { id, name } => {
    //             writeln!(fs, "NewSpan: {id} {name}").unwrap();
    //         }
    //         ProfilingEvent::Enter {
    //             hart_id,
    //             id,
    //             instant,
    //         } => writeln!(fs, "Enter: {hart_id} {id} {instant}").unwrap(),
    //         ProfilingEvent::Exit { id, instant } => writeln!(fs, "Exit: {id}
    // {instant}").unwrap(),     }
    // }
}
