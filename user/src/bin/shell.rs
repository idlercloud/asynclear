#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user;

const LF: u8 = 0x0au8;
const CR: u8 = 0x0du8;
const DL: u8 = 0x7fu8;
const BS: u8 = 0x08u8;

use alloc::{string::String, vec::Vec};

use user::{console::getchar, exec, flush, fork, waitpid};

#[no_mangle]
pub fn main() -> i32 {
    println!("Rust user shell");
    let mut line: String = String::new();
    print!(">> ");
    flush();
    loop {
        let c = getchar();
        match c {
            LF | CR => {
                println!("");
                if !line.is_empty() {
                    if line == "exit" {
                        return 0;
                    }
                    let mut args = line
                        .as_str()
                        .split(' ')
                        .map(String::from)
                        .collect::<Vec<_>>();

                    for s in args.iter_mut() {
                        s.push('\0');
                    }

                    let mut args_addr: Vec<*const u8> =
                        args.iter().map(|arg| arg.as_ptr()).collect();
                    args_addr.push(core::ptr::null());
                    let pid = fork();
                    if pid == 0 {
                        // child process
                        if exec(args[0].as_str(), args_addr.as_slice()) < 0 {
                            println!("Error when executing!");
                            return -4;
                        }
                        unreachable!();
                    } else {
                        let mut exit_code: i32 = 0;
                        let exit_pid = waitpid(pid as usize, &mut exit_code);
                        assert_eq!(pid, exit_pid);
                        println!("Shell: Process {} exited with code {}", pid, exit_code);
                    }
                    line.clear();
                }
                print!(">> ");
                flush();
            }
            BS | DL => {
                if !line.is_empty() {
                    print!("{}", BS as char);
                    print!(" ");
                    print!("{}", BS as char);
                    flush();
                    line.pop();
                }
            }
            _ => {
                print!("{}", c as char);
                flush();
                line.push(c as char);
            }
        }
    }
}
