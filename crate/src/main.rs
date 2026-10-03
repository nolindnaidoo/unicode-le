mod cli;
mod detect;
mod escape;
mod mcp;
mod scan;
mod walk;

#[cfg(test)]
mod tables;
#[cfg(test)]
mod testing;

fn main() -> std::process::ExitCode {
    cli::run()
}
