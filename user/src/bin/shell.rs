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
use core::ffi::CStr;

use user::{chdir, console::getchar, exec, exit, flush, fork, waitpid};

#[no_mangle]
pub fn main() -> i32 {
    println!("Rust user shell");
    let mut line = String::new();
    print!(">> ");
    flush();
    loop {
        handle_input(&mut line);
    }
}

fn handle_input(line: &mut String) {
    let c = getchar();
    match c {
        LF | CR => {
            handle_line_end(line);
        }
        BS | DL => {
            handle_delete(line);
        }
        _ => {
            print!("{}", c as char);
            flush();
            line.push(c as char);
        }
    }
}

fn handle_line_end(line: &mut String) {
    println!("");
    let trimed = line.trim();
    if !trimed.is_empty() {
        let mut args = trimed.split(' ').map(String::from).collect::<Vec<_>>();

        if !handle_builtin(&mut args) {
            for s in args.iter_mut() {
                s.push('\0');
            }

            let mut args_addr: Vec<*const u8> = args.iter().map(|arg| arg.as_ptr()).collect();
            args_addr.push(core::ptr::null());
            let pid = fork();
            if pid < 0 {
                println!("Error when forking");
                return;
            }

            if pid == 0 {
                let ret = exec(
                    CStr::from_bytes_with_nul(args[0].as_bytes()).unwrap(),
                    args_addr.as_slice(),
                );
                if ret < 0 {
                    println!("Error when executing!");
                    exit(-4);
                }
                unreachable!();
            } else {
                let mut exit_code: i32 = 0;
                let exit_pid = waitpid(pid as usize, &mut exit_code);
                assert_eq!(pid, exit_pid);
                println!("Shell: Process {} exited with code {}", pid, exit_code);
            }
        }
    }
    line.clear();
    print!(">> ");
    flush();
}

fn handle_delete(line: &mut String) {
    if !line.is_empty() {
        print!("{}", BS as char);
        print!(" ");
        print!("{}", BS as char);
        flush();
        line.pop();
    }
}

fn handle_builtin(args: &mut [String]) -> bool {
    if args[0] == "exit" {
        handle_builtin_exit(args);
        return true;
    } else if args[0] == "cd" {
        handle_builtin_cd(args);
        return true;
    }

    false
}

fn handle_builtin_exit(args: &[String]) {
    let mut exit_code = 0;
    if args.len() > 2 {
        println!("Unknown extra args");
        return;
    }
    if args.len() == 2 {
        if let Ok(parsed) = args[1].parse::<i8>() {
            exit_code = parsed;
        } else {
            println!("Not valid exit code");
            return;
        }
    }
    exit(exit_code as i32);
}

fn handle_builtin_cd(args: &mut [String]) {
    if args.len() > 2 {
        println!("Unknown extra args");
        return;
    }
    if args.len() < 2 {
        println!("Missing args");
        return;
    }
    args[1].push('\0');
    if chdir(CStr::from_bytes_with_nul(args[1].as_bytes()).unwrap()) < 0 {
        println!("Error when change dir to {}", args[1]);
    }
}
