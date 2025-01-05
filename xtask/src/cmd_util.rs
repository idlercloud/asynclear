use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::{self, Child, Stdio},
};

pub struct Cmd(process::Command);

impl Cmd {
    pub fn new(name: impl AsRef<OsStr>) -> Self {
        Self(process::Command::new(name))
    }

    pub fn parse(cmd_line: impl AsRef<str>) -> Self {
        let cmd_line = cmd_line.as_ref();
        let mut component = cmd_line.split_whitespace();
        let name = component.next().unwrap();
        let mut cmd = Self(process::Command::new(name));
        cmd.0.args(component);
        cmd
    }

    pub fn arg(&mut self, s: impl AsRef<OsStr>) -> &mut Self {
        self.0.arg(s);
        self
    }

    pub fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> &mut Self {
        self.0.args(args);
        self
    }

    pub fn optional_arg(&mut self, option_arg: Option<impl AsRef<OsStr>>) -> &mut Self {
        if let Some(arg) = option_arg {
            self.0.arg(arg);
        }
        self
    }

    pub fn optional_args(&mut self, option_args: Option<impl IntoIterator<Item = impl AsRef<OsStr>>>) -> &mut Self {
        if let Some(args) = option_args {
            self.0.args(args);
        }
        self
    }

    pub fn envs(&mut self, vars: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>) -> &mut Self {
        self.0.envs(vars);
        self
    }

    #[expect(unused)]
    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Self {
        self.0.current_dir(dir);
        self
    }

    #[track_caller]
    pub fn status(&mut self) -> process::ExitStatus {
        match self.0.status() {
            Ok(status) => status,
            Err(e) => {
                panic!("Failed calling {:?}: {e}", self.info());
            }
        }
    }

    pub fn info(&self) -> OsString {
        let mut msg = OsString::new();
        if let Some(dir) = self.0.get_current_dir() {
            msg.push("cd ");
            msg.push(dir);
            msg.push("; ");
        }
        msg.push(self.0.get_program());
        for a in self.0.get_args() {
            msg.push(" ");
            msg.push(a);
        }
        for (k, v) in self.0.get_envs() {
            msg.push(" ");
            msg.push(k);
            if let Some(v) = v {
                msg.push("=");
                msg.push(v);
            }
        }
        msg
    }

    pub fn invoke(&mut self) -> &mut Self {
        let status = self.status();
        if !status.success() {
            panic!("Failed with code {}: {:?}", status.code().unwrap(), self.info());
        }
        self
    }

    pub fn output(&mut self) -> process::Output {
        let output = self.0.output().unwrap();
        if !output.status.success() {
            panic!(
                "Failed calling {:?}: error code {}",
                self.info(),
                output.status.code().unwrap(),
            );
        }
        output
    }

    pub fn spawn(&mut self) -> Child {
        self.0.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap()
    }
}
