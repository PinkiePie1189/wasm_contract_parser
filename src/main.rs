use std::{collections::HashMap, env::args, fs::File, io::BufWriter, path::PathBuf};
use regex::Regex;
use wabt::Wasm2Wat;
use serde_json::to_writer_pretty;  // or `to_writer` for compact output

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = args().collect();
    if args.len() < 2 {
        panic!("Usage {} <WASM file>", args[0]);
    }
    let wasm_bytes = std::fs::read(args[1].clone())?;    
    let buf = Wasm2Wat::new().convert(&wasm_bytes)?;
    let s = String::from_utf8(buf.as_ref().to_vec()).unwrap();

    let mut exports: HashMap<String, String> = HashMap::new();
    {
        let export_regex = r#"\(export "(?P<name>\w+)" \(func (?P<id>\d+)\)"#;
        let regex = Regex::new(export_regex)?;
        let caps = regex.captures_iter(&s);

        for cap in caps {
            exports.insert(cap["id"].to_string(), cap["name"].to_string());
        }
    }

    let stub_sequence_regex = Regex::new(r"i32\.const (\d+)\s*i32\.const 25\s*call (\d+)\s*unreachable\)")?;
    let func_header_regex = Regex::new(r"\(func \(;(?P<id>\d+);\) \(type (\d+)\) \(param i32\)")?;

    let mut partial_abi: HashMap<String, i32> = HashMap::new();

    match stub_sequence_regex.find(&s) {
        Some(m) => {
            let slice = &s[..m.end()];

            match func_header_regex.captures_iter(slice).last() {
                Some(cap) => {
                    let stub_id = &cap["id"];
                    let ids = exports.keys().into_iter();
                    ids.for_each(|id| {
                        let func_regex = format!(r"(?s)\(func \(;{};\) \(type \d+\).*?i32.const (?P<arity>\d+)\s*call {}", id, stub_id);
                        let func_regex_compiled = Regex::new(&func_regex).unwrap();
                        if let Some(cap_func) = func_regex_compiled.captures(&s) {
                            let arity = &cap_func["arity"];

                            partial_abi.insert(exports[id].clone(), arity.parse::<i32>().unwrap());
                            println!("Func name: {} id: {} arity {}", exports[id], id, arity);
                        } else {
                            eprintln!("Can't find checkNumArguments call in func  name: {} id: {}", exports[id], id);
                        }
                    });

                    let mut path_buf = PathBuf::from(args[1].clone());
                    path_buf.set_extension("abi.json");
                    match File::create(path_buf) {
                        Ok(file) => {
                            let writer = BufWriter::new(file);
                            to_writer_pretty(writer, &partial_abi).map_err(|e|  format!("Can't write json abi, error {}", e))?;
                            Ok(())
                        },
                        Err(e) => Err(format!("Can't create file for writing, error {}", e).into()),
                    }
                },
                None => Err("Can't find function start".into())
            }
        },
        None => Err("Can't find checkNumArguments stub".into())
    }
}
