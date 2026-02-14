use std::process::Command;

pub fn play_start_sound() {
    let _ = Command::new("afplay")
        .arg("/System/Library/Sounds/Tink.aiff")
        .spawn();
}

pub fn play_stop_sound() {
    let _ = Command::new("afplay")
        .arg("/System/Library/Sounds/Pop.aiff")
        .spawn();
}
