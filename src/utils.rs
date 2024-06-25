use bincode::enc::write::Writer;
use cairo1_run::{cairo_run_program, Cairo1RunConfig, error::Error, FuncArg};
use cairo_lang_compiler::{compile_prepared_db, db::RootDatabase, project::setup_project, CompilerConfig};
use cairo_vm::{types::layout_name::LayoutName, vm::{self, errors::trace_errors::TraceError, trace::trace_entry::RelocatedTraceEntry}, Felt252};
use hyle_contract::HyleOutput;
use num::BigInt;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::{self, BufWriter, Write}, path::PathBuf};

pub mod error;

#[derive(Serialize, Deserialize)]
pub struct CairoRunOutput {
    pub trace: Vec<u8>,
    pub memory: Vec<u8>,
    pub output: HyleOutput<Event>
}

pub fn cairo_run(serialized_sierra_program: &str, program_inputs: &str) -> Result<CairoRunOutput, error::RunnerError> {
    let sierra_program = serde_json::from_str(&serialized_sierra_program)?;
    let inputs = process_args(&program_inputs).unwrap();

    let cairo_run_config = Cairo1RunConfig {
        args: &inputs.0,
        serialize_output: true,
        trace_enabled: true,
        relocate_mem: true,
        layout: LayoutName::all_cairo,
        proof_mode: true,
        finalize_builtins: false,
        append_return_values: false,
    };

    let (runner, _, serialized_output) = cairo_run_program(&sierra_program, cairo_run_config)?;

    let relocated_trace: Vec<RelocatedTraceEntry> = runner
        .relocated_trace
        .ok_or(Error::Trace(TraceError::TraceNotRelocated))?;

    let deser = <HyleOutput<Event> as DeserializableHyleOutput>::deserialize(&serialized_output.unwrap());

    let cairo_run_output = CairoRunOutput{
        trace: encode_trace(&relocated_trace),
        memory: encode_memory(&runner.relocated_memory),
        output: deser
    };

    Ok(cairo_run_output)
}


pub fn cairo_run_from_cli(trace_bin_path: &str, memory_bin_path: &str, program_inputs_path: &str, cairo_program_path: &str, output_path: &str, sierra_path: &str) -> Result<(), error::RunnerError> {

    // Try to parse the file as a sierra program
    let program_inputs = std::fs::read_to_string(program_inputs_path)?;

    // Compile cairo program
    let compiler_config = CompilerConfig {
        replace_ids: true,
        ..CompilerConfig::default()
    };
    let mut db = RootDatabase::builder()
        .detect_corelib()
        .skip_auto_withdraw_gas()
        .build()
        .unwrap();
    let main_crate_ids = setup_project(&mut db, &PathBuf::from(cairo_program_path)).unwrap();
    let sierra_program = compile_prepared_db(&mut db, main_crate_ids, compiler_config).unwrap();


    // TODO: le save dans un fichier pour que le front puisse l'utiliser en static
    // let serialized_sierra_program: Vec<u8> = bincode::serde::encode_to_vec(&sierra_program, bincode::config::standard())?;
    let serialized_sierra_program: String = serde_json::to_string(&sierra_program)?;
    // Save serialized sierra program
    let file = std::fs::File::create(&sierra_path)?;
    let mut sierra_writer = BufWriter::new(file);
    serde_json::to_writer(&mut sierra_writer, &serialized_sierra_program).expect("failed to save sierra program");
    sierra_writer.flush()?;


    let cairo_run_output = cairo_run(&serialized_sierra_program, &program_inputs)?;

    // Save trace file
    let trace_file = std::fs::File::create(trace_bin_path)?;
    let mut trace_writer = FileWriter::new(io::BufWriter::with_capacity(3 * 1024 * 1024, trace_file));
    trace_writer.write(&cairo_run_output.trace)?;
    trace_writer.flush()?;

    // Save memory file
    let memory_file = std::fs::File::create(memory_bin_path)?;
    let mut memory_writer =  FileWriter::new(io::BufWriter::with_capacity(5 * 1024 * 1024, memory_file));
    memory_writer.write(&cairo_run_output.memory)?;
    memory_writer.flush()?;

    // Save output file
    let file = File::create(output_path)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer(&mut writer, &cairo_run_output.output)?;
    writer.flush()?;

    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct FuncArgs(pub Vec<FuncArg>);

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

pub struct FileWriter {
    buf_writer: io::BufWriter<std::fs::File>,
    bytes_written: usize,
}

#[derive(Deserialize, Serialize)]
pub struct Event {
    from: String,
    to: String,
    amount: u64,
}

pub trait DeserializableHyleOutput {
    fn i_to_w(s: String) -> String;
    fn deserialize_cairo_bytesarray(data: &mut Vec<&str>) -> String;
    fn deserialize(input: &str) -> Self;
}


impl DeserializableHyleOutput for HyleOutput<Event> {
    /// Receives an int, change base to hex, decode it to ascii
    fn i_to_w(s: String) -> String {
        let int = s.parse::<BigInt>().expect("failed to parse the address");
        let hex = hex::decode(format!("{:x}", int)).expect("failed to parse the address");
        String::from_utf8(hex).expect("failed to parse the address")
    }

    /// BytesArray serialisation is composed of 3 values (if the data is less than 31bytes)
    /// https://github.com/starkware-libs/cairo/blob/main/corelib/src/byte_array.cairo#L24-L34
    /// WARNING: Deserialization is not yet robust.
    /// TODO: pending_word_len not used.
    /// TODO: add checking on inputs.
    fn deserialize_cairo_bytesarray(data: &mut Vec<&str>) -> String {
        let pending_word = data.remove(0).parse::<usize>().unwrap();
        let _pending_word_len = data.remove(pending_word + 1).parse::<usize>().unwrap();
        let mut word: String = "".into();
        for _ in 0..pending_word+1 {
            let d: String = data.remove(0).into();
            if d != "0"{
                word.push_str(&Self::i_to_w(d));
            }
        }
        word
    }

    /// Deserialize the output of the cairo erc20 contract.
    /// elements for the "from" address
    /// elements for the "to" address
    /// [-2] element for the amount transfered
    /// [-1] element for the next state
    fn deserialize(input: &str) -> Self {
        let trimmed = input.trim_matches(|c| c == '[' || c == ']');
        let mut parts: Vec<&str> = trimmed.split_whitespace().collect();
        // extract version
        let version = parts.remove(0).parse::<u32>().unwrap();
        // extract initial_state
        let initial_state: String = parts.remove(0).parse::<String>().unwrap();
        // extract next_state
        let next_state: String = parts.remove(0).parse::<String>().unwrap();
        // extract origin
        let origin: String = Self::deserialize_cairo_bytesarray(&mut parts);
        // extract caller
        let caller: String = Self::deserialize_cairo_bytesarray(&mut parts);
        // extract tx_hash
        let tx_hash: String = parts.remove(0).parse::<String>().unwrap();
        // extract from
        let from = Self::deserialize_cairo_bytesarray(&mut parts);
        // extract to
        let to = Self::deserialize_cairo_bytesarray(&mut parts);
        // extract amount
        let amount = parts.remove(0).parse::<u64>().unwrap();

        HyleOutput {
            version,
            initial_state: initial_state.as_bytes().to_vec(),
            next_state: next_state.as_bytes().to_vec(),
            origin,
            caller,
            block_number: 0,
            block_time: 0,
            tx_hash: tx_hash.as_bytes().to_vec(),
            program_outputs: Event {from, to, amount}
        }
    }
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
    pub fn new(buf_writer: io::BufWriter<std::fs::File>) -> Self {
        Self {
            buf_writer,
            bytes_written: 0,
        }
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.buf_writer.flush()
    }
}

/// Parses a string of ascii whitespace separated values, containing either numbers or series of numbers wrapped in brackets
/// Returns an array of felts and felt arrays
pub fn process_args(value: &str) -> Result<FuncArgs, String> {
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
pub fn process_array<'a>(iter: &mut impl Iterator<Item = &'a str>) -> Result<FuncArg, String> {
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

/// Writes the trace binary representation.
///
/// Bincode encodes to little endian by default and each trace entry is composed of
/// 3 usize values that are padded to always reach 64 bit size.
pub fn encode_trace(relocated_trace: &[vm::trace::trace_entry::RelocatedTraceEntry]) -> Vec<u8> {
    let mut trace_bytes: Vec<u8> = vec![];
    for (i, entry) in relocated_trace.iter().enumerate() {
        trace_bytes.extend(&((entry.ap as u64).to_le_bytes()));
        trace_bytes.extend(&((entry.fp as u64).to_le_bytes()));
        trace_bytes.extend(&((entry.pc as u64).to_le_bytes()));
    }
    trace_bytes
}

/// Writes a binary representation of the relocated memory.
///
/// The memory pairs (address, value) are encoded and concatenated:
/// * address -> 8-byte encoded
/// * value -> 32-byte encoded
pub fn encode_memory(relocated_memory: &[Option<Felt252>]) -> Vec<u8>{
    let mut memory_bytes: Vec<u8> = vec![];
    for (i, memory_cell) in relocated_memory.iter().enumerate() {
        match memory_cell {
            None => continue,
            Some(unwrapped_memory_cell) => {
                memory_bytes.extend(&(i as u64).to_le_bytes());
                memory_bytes.extend(&unwrapped_memory_cell.to_bytes_le());
            }
        }
    }
    memory_bytes
}
