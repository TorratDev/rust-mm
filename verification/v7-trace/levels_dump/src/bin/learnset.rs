// Prints the level-up learnset of every species as JSON lines: {"species":N,"level_up":[[move,level],...]}.
// Scratch output for levelup_probe.py (which move a forced learner can learn at which level); not repo data.
use rescue_data::moves::parse_moves;
use rescue_formats::archive::Archive;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

fn main() {
    let path = std::env::args().nth(1).expect("usage: learnset ROM");
    let rom = std::fs::read(path).expect("read rom");
    let header = Header::parse(&rom).unwrap();
    let fs = Filesystem::parse(&rom, &header).unwrap();
    let sys = fs.data(&rom, fs.get("system.sbin").unwrap().id).unwrap().to_vec();
    let arc = Archive::parse(&sys).unwrap();
    let entry = arc.entries().iter().find(|e| e.name == "wazapara").expect("wazapara");
    let md = parse_moves(arc.data(&sys, entry).unwrap()).unwrap();
    for (species, ls) in md.learnsets.iter().enumerate() {
        let body: Vec<String> = ls.level_up.iter().map(|(m, l)| format!("[{m},{l}]")).collect();
        println!("{{\"species\":{species},\"level_up\":[{}]}}", body.join(","));
    }
}
