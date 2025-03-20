use pusu_toolbox::run;

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {}", err);
    }
}
