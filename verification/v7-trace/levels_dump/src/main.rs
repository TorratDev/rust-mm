// Prints the parse_level_data records of every lvmpNNN entry as JSON lines:
// {"species":N,"records":[[exp,hp,atk0,atk1,def0,def1],...]}. The output is a scratch file (the table values
// stay out of the repo) and is the LEVELS input of levelup_probe.py.
use rescue_data::species::parse_level_data;
use rescue_formats::archive::Archive;
use rescue_nds::fs::Filesystem;
use rescue_nds::header::Header;

fn main() {
    let path = std::env::args().nth(1).expect("usage: levels-dump ROM");
    let rom = std::fs::read(path).expect("read rom");
    let header = Header::parse(&rom).unwrap();
    let fs = Filesystem::parse(&rom, &header).unwrap();
    let sys = fs.data(&rom, fs.get("system.sbin").unwrap().id).unwrap().to_vec();
    let arc = Archive::parse(&sys).unwrap();
    for e in arc.entries().iter().filter(|e| e.name.starts_with("lvmp")) {
        let species: u32 = e.name[4..].parse().unwrap();
        let recs = parse_level_data(arc.data(&sys, e).unwrap()).unwrap();
        let body: Vec<String> = recs.iter().map(|r| format!("[{},{},{},{},{},{}]",
            r.exp_required, r.hp, r.attack[0], r.attack[1], r.defense[0], r.defense[1])).collect();
        println!("{{\"species\":{species},\"records\":[{}]}}", body.join(","));
    }
}
