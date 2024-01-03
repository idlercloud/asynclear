use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
    path::PathBuf,
};

use clap::Parser;
use serde_json::{json, Map};

/// 从内核日志的 profiling report 中进行分析，并输出 Google Trace Event 格式的记录
#[derive(Parser)]
pub struct ProfilingArgs {
    /// 输入的 Log 文件的路径
    #[clap(short, long)]
    input: PathBuf,
    /// 输入的 json 文件的路径
    #[clap(short, long)]
    output: PathBuf,
}

impl ProfilingArgs {
    pub fn analyze(self) {
        let mut lines = BufReader::new(File::open(self.input).unwrap())
            .lines()
            .enumerate();
        while let Some((_, line)) = lines.next() {
            let line = line.unwrap();
            if line == "<Profiling Report>" {
                break;
            }
        }
        use serde_json::Value;
        let mut json = Value::Object(Map::new());
        json["displayTimeUnit"] = Value::String("ns".to_string());
        let mut trace_events = Vec::new();
        let mut spans = Vec::new();
        while let Some((line_num, line)) = lines.next() {
            let line = line.unwrap();
            let Some(pos) = line.find(':') else {
                break;
            };
            let mut res = line[pos + 1..].trim().split(' ');

            match &line[..pos] {
                "NewSpan" => {
                    let id = res.next().unwrap();
                    let name = line[pos + 1..].trim()[id.len()..].trim();
                    let id: usize = id.parse().unwrap();
                    if id >= spans.len() {
                        spans.resize(id + 1, (String::new(), 0));
                    }
                    spans[id] = (name.to_string(), 0);
                }
                "Enter" => {
                    let hart_id: usize = res.next().unwrap().parse().unwrap();
                    let id: usize = res.next().unwrap().parse().unwrap();
                    spans[id].1 = hart_id;
                    let timestamp: usize = res.next().unwrap().parse().unwrap();
                    trace_events.push(json!({
                        "name": spans[id].0,
                        "cat": "inst",
                        "ph": "B",
                        "ts": timestamp,
                        "pid": 0,
                        "tid": hart_id,
                    }));
                }
                "Exit" => {
                    let id: usize = res.next().unwrap().parse().unwrap();
                    let hart_id = spans[id].1;
                    let timestamp: usize = res.next().unwrap().parse().unwrap();
                    trace_events.push(json!({
                        "ph": "E",
                        "ts": timestamp,
                        "pid": 0,
                        "tid": hart_id,
                    }));
                }
                other => {
                    panic!("Unexpected type {other} in line {}: {line}", line_num + 1);
                }
            }
        }
        json["traceEvents"] = Value::Array(trace_events);
        fs::write(self.output, json.to_string()).unwrap();
    }
}
