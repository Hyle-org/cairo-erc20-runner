use utils::cairo_run;
use clap::Parser;
use crate::utils::error::RunnerError;

mod utils;

#[derive(Parser)]
struct Cli {
    cairo_program_path: String,
    program_inputs_path: String,
    trace_bin_path: String,
    memory_bin_path: String,
    output_path: String,
}


fn main() -> Result<(), RunnerError>{
    let cli_args = Cli::parse();

    let res = cairo_run(
        &cli_args.trace_bin_path,
        &cli_args.memory_bin_path,
        &cli_args.program_inputs_path,
        &cli_args.cairo_program_path,
        &cli_args.output_path
    );
    match res {
        Result::Ok(()) => (),
        Result::Err(err) => {
            return Err(err);
        }
    };
    Ok(())
}