use std::fs;

fn main() {
    let output = "ascii and ascii table\n\n+---+\n| A |\n+---+\n";
    let _ = fs::write("/out/generated.txt", output);
}
