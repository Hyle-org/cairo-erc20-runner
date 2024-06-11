use bincode::enc::write::Writer;
use cairo1_run::{cairo_run_program, Cairo1RunConfig, error::Error, FuncArg};
use cairo_lang_compiler::{compile_prepared_db, db::RootDatabase, project::setup_project, CompilerConfig};
use cairo_vm::{air_public_input::PublicInputError, types::layout_name::LayoutName, vm::errors::trace_errors::TraceError, Felt252};
use serde_json;
use serde::Serialize;
use std::{env, io::{self, Write}, path::PathBuf};

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        eprintln!("Usage: {} <cairo_program_path> <trace_path> <memory_path> <program_inputs_path>", args[0]);
        std::process::exit(1);
    }

    let cairo_program_path = &args[1];
    let trace_path = &args[2];
    let memory_path = &args[3];
    let program_inputs_path = &args[4];

    let args = Args {
        trace_file: Some(PathBuf::from(trace_path)),
        memory_file: Some(PathBuf::from(memory_path)),
        layout: "all_cairo".to_string(),
        proof_mode: true,
        air_public_input: None,
        air_private_input: None,
        cairo_pie_output: None,
        args: process_args(&std::fs::read_to_string(program_inputs_path)?).unwrap(),
        print_output: false,
        append_return_values: false,
    };
    
    // Try to parse the file as a sierra program
    let file = std::fs::read(&cairo_program_path).expect("Failed to load cairo file");
    let sierra_program = match serde_json::from_slice(&file) {
        Ok(program) => program,
        Err(_) => {
            // If it fails, try to compile it as a cairo program
            let compiler_config = CompilerConfig {
                replace_ids: true,
                ..CompilerConfig::default()
            };
            let mut db = RootDatabase::builder()
                .detect_corelib()
                .skip_auto_withdraw_gas()
                .build()
                .unwrap();
            let main_crate_ids = setup_project(&mut db, &PathBuf::from(&cairo_program_path)).unwrap();
            compile_prepared_db(&mut db, main_crate_ids, compiler_config).unwrap()
        }
    };


    let cairo_run_config = Cairo1RunConfig {
        args: &args.args.0, //inputs
        serialize_output: true,
        trace_enabled: true,
        relocate_mem: true,
        layout: LayoutName::all_cairo,
        proof_mode: true,
        finalize_builtins: false, // TODO: investigate if we want it ?
        append_return_values: false, // TODO: investigate if we want it ?
    };
    
    let (runner, _, serialized_output) = cairo_run_program(&sierra_program, cairo_run_config)?;

    if let Some(file_path) = args.air_public_input {
        let json = runner.get_air_public_input()?.serialize_json()?;
        std::fs::write(file_path, json)?;
    }

    if let (Some(file_path), Some(trace_file), Some(memory_file)) = (
        args.air_private_input,
        args.trace_file.clone(),
        args.memory_file.clone(),
    ) {
        // Get absolute paths of trace_file & memory_file
        let trace_path = trace_file
            .as_path()
            .canonicalize()
            .unwrap_or(trace_file.clone())
            .to_string_lossy()
            .to_string();
        let memory_path = memory_file
            .as_path()
            .canonicalize()
            .unwrap_or(memory_file.clone())
            .to_string_lossy()
            .to_string();

        let json = runner
            .get_air_private_input()
            .to_serializable(trace_path, memory_path)
            .serialize_json()
            .map_err(PublicInputError::Serde)?;
        std::fs::write(file_path, json)?;
    }

    if let Some(ref file_path) = args.cairo_pie_output {
        runner.get_cairo_pie()?.write_zip_file(file_path)?
    }

    if let Some(trace_path) = args.trace_file {
        let relocated_trace = runner
            .relocated_trace
            .ok_or(Error::Trace(TraceError::TraceNotRelocated))?;
        let trace_file = std::fs::File::create(trace_path)?;
        let mut trace_writer =
            FileWriter::new(io::BufWriter::with_capacity(3 * 1024 * 1024, trace_file));

        cairo_vm::cairo_run::write_encoded_trace(&relocated_trace, &mut trace_writer)?;
        trace_writer.flush()?;
    }
    if let Some(memory_path) = args.memory_file {
        let memory_file = std::fs::File::create(memory_path)?;
        let mut memory_writer =
            FileWriter::new(io::BufWriter::with_capacity(5 * 1024 * 1024, memory_file));

        cairo_vm::cairo_run::write_encoded_memory(&runner.relocated_memory, &mut memory_writer)?;
        memory_writer.flush()?;
    }

    let deser = HyleOutput::deserialize(&serialized_output.unwrap());
    println!("{:?}", serde_json::to_string(&deser).unwrap());

    Ok(())
}


#[derive(Debug, Clone, Default)]
pub struct FuncArgs(Vec<FuncArg>);

#[derive(Debug)]
pub struct Args {
    pub trace_file: Option<PathBuf>,
    pub memory_file: Option<PathBuf>,
    pub layout: String,
    pub proof_mode: bool,
    pub air_public_input: Option<PathBuf>,
    pub air_private_input: Option<PathBuf>,
    pub cairo_pie_output: Option<PathBuf>,
    pub args: FuncArgs,
    pub print_output: bool,
    pub append_return_values: bool,
}

struct FileWriter {
    buf_writer: io::BufWriter<std::fs::File>,
    bytes_written: usize,
}

impl Writer for FileWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        self.buf_writer
            .write_all(bytes)
            .map_err(|e| bincode::error::EncodeError::Io {
                inner: e,
                index: self.bytes_written,
            })?;

        self.bytes_written += bytes.len();

        Ok(())
    }
}

impl FileWriter {
    fn new(buf_writer: io::BufWriter<std::fs::File>) -> Self {
        Self {
            buf_writer,
            bytes_written: 0,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf_writer.flush()
    }
}

/// Parses a string of ascii whitespace separated values, containing either numbers or series of numbers wrapped in brackets
/// Returns an array of felts and felt arrays
fn process_args(value: &str) -> Result<FuncArgs, String> {
    let mut args = Vec::new();
    // Split input string into numbers and array delimiters
    let mut input = value.split_ascii_whitespace().flat_map(|mut x| {
        // We don't have a way to split and keep the separate delimiters so we do it manually
        let mut res = vec![];
        if let Some(val) = x.strip_prefix('[') {
            res.push("[");
            x = val;
        }
        if let Some(val) = x.strip_suffix(']') {
            if !val.is_empty() {
                res.push(val)
            }
            res.push("]")
        } else if !x.is_empty() {
            res.push(x)
        }
        res
    });
    // Process iterator of numbers & array delimiters
    while let Some(value) = input.next() {
        match value {
            "[" => args.push(process_array(&mut input)?),
            _ => args.push(FuncArg::Single(
                Felt252::from_dec_str(value)
                    .map_err(|_| format!("\"{}\" is not a valid felt", value))?,
            )),
        }
    }
    Ok(FuncArgs(args))
}

/// Processes an iterator of format [s1, s2,.., sn, "]", ...], stopping at the first "]" string
/// and returning the array [f1, f2,.., fn] where fi = Felt::from_dec_str(si)
fn process_array<'a>(iter: &mut impl Iterator<Item = &'a str>) -> Result<FuncArg, String> {
    let mut array = vec![];
    for value in iter {
        match value {
            "]" => break,
            _ => array.push(
                Felt252::from_dec_str(value)
                    .map_err(|_| format!("\"{}\" is not a valid felt", value))?,
            ),
        }
    }
    Ok(FuncArg::Array(array))
}

#[derive(Serialize)]
struct Event {
    from: String,
    to: String,
    amount: i64,
}

#[derive(Serialize)]
struct HyleOutput {
    event: Event,
    next_state: String,
}

impl HyleOutput {
    /// BytesArray serialisation is composed of 3 values (if the data is less than 31bytes)
    /// https://github.com/starkware-libs/cairo/blob/main/corelib/src/byte_array.cairo#L24-L34
    /// TODO: Make deserialization adaptable to words >31bytes
    fn deserialize_cairo_bytesarray(_pending_word: String, data: String, _word_len: String) -> String {
        let data_int = data.parse::<i32>().expect("failed to parse the address");
        let data_hex = hex::decode(format!("{:x}", data_int)).expect("failed to parse the address");
        String::from_utf8(data_hex).expect("failed to parse the address")
    }

    /// Deserialize the output of the cairo erc20 contract.
    /// [0:2] elements will be the "from" address
    /// [3:5] elements will be the "to" address
    /// [6] element will be the amount transfered
    /// [7] element will be the next state
    fn deserialize(input: &str) -> Self {
        let trimmed = input.trim_matches(|c| c == '[' || c == ']');
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        // let parts_strings: Vec<String> = parts.iter().map(|&s|s.into()).collect();

        let from_pending_word: String = parts[0].into();
        let from_data: String = parts[1].into();
        let from_word_len: String = parts[2].into();

        let to_pending_word: String = parts[3].into();
        let to_data: String = parts[4].into();
        let to_word_len: String = parts[5].into();

        let from = Self::deserialize_cairo_bytesarray(from_pending_word, from_data, from_word_len);
        let to = Self::deserialize_cairo_bytesarray(to_pending_word, to_data, to_word_len);
        let amount = parts[6].parse::<i64>().unwrap();
        let next_state: String = parts[7].into();
        HyleOutput {
            event: Event {from: from.into(), to: to.into(), amount},
            next_state
        }
    }
}
