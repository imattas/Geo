use clap::Parser;
use geo::cli::Cli;

fn main() {
    let cli = Cli::parse();

    if let Err(diagnostics) = geo::driver::run_cli(cli) {
        for diagnostic in diagnostics {
            eprintln!("{}", diagnostic.render());
        }
        std::process::exit(1);
    }
}
