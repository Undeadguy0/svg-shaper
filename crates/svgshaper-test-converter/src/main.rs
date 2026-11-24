use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

fn main() -> ExitCode {
    if let Ok(log_path) = env::var("SVGSHAPER_ARGV_LOG") {
        if !log_path.is_empty() {
            let lines: Vec<String> = env::args_os()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            let _ = fs::write(log_path, lines.join("\n") + "\n");
        }
    }

    let opts = match parse_args() {
        Ok(opts) => opts,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };

    let mut sleep_ms = opts.sleep_ms;
    let source_text = opts.source.to_string_lossy();
    for (needle, millis) in &opts.sleep_rules {
        if source_text.contains(needle) {
            sleep_ms = sleep_ms.max(*millis);
        }
    }
    if sleep_ms > 0 {
        thread::sleep(Duration::from_millis(sleep_ms));
    }

    if opts.exit_code != 0 {
        let _ = fs::write(&opts.output, b"not-a-png");
        return ExitCode::from(opts.exit_code as u8);
    }

    if let Err(err) = write_png(&opts.output, opts.width, opts.height) {
        eprintln!("{err}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

struct Opts {
    width: u32,
    height: u32,
    output: PathBuf,
    source: PathBuf,
    sleep_ms: u64,
    exit_code: i32,
    sleep_rules: Vec<(String, u64)>,
}

fn parse_args() -> Result<Opts, String> {
    let mut width = None;
    let mut height = None;
    let mut output = None;
    let mut source = None;
    let mut sleep_ms = env::var("SVGSHAPER_SLEEP_MS")
        .ok()
        .and_then(|text| text.parse().ok());
    let mut exit_code = env::var("SVGSHAPER_EXIT_CODE")
        .ok()
        .and_then(|text| text.parse().ok());
    let mut sleep_rules = Vec::new();

    let args: Vec<String> = env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--width" => width = Some(need_u32(&args, &mut index, "--width")?),
            "--height" => height = Some(need_u32(&args, &mut index, "--height")?),
            "--format" => {
                let format = need_value(&args, &mut index, "--format")?;
                if !format.eq_ignore_ascii_case("png") {
                    return Err(format!("ожидался формат png, получен {format}"));
                }
            }
            "--output" => output = Some(PathBuf::from(need_value(&args, &mut index, "--output")?)),
            "--sleep-ms" => sleep_ms = Some(need_u64(&args, &mut index, "--sleep-ms")?),
            "--exit-code" => exit_code = Some(need_i32(&args, &mut index, "--exit-code")?),
            "--sleep-if" => {
                let rule = need_value(&args, &mut index, "--sleep-if")?;
                let (needle, millis) = rule
                    .rsplit_once('=')
                    .ok_or_else(|| format!("--sleep-if ждёт needle=millis, получено {rule}"))?;
                let millis = millis
                    .parse()
                    .map_err(|_| format!("нечисловая пауза в --sleep-if: {millis}"))?;
                sleep_rules.push((needle.to_string(), millis));
            }
            other if other.starts_with('-') => {
                return Err(format!("неизвестный аргумент {other}"));
            }
            other => source = Some(PathBuf::from(other)),
        }
        index += 1;
    }

    Ok(Opts {
        width: width.ok_or("нет --width")?,
        height: height.ok_or("нет --height")?,
        output: output.ok_or("нет --output")?,
        source: source.ok_or("нет пути SVG")?,
        sleep_ms: sleep_ms.unwrap_or(0),
        exit_code: exit_code.unwrap_or(0),
        sleep_rules,
    })
}

fn need_value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("нет значения у {flag}"))
}

fn need_u32(args: &[String], index: &mut usize, flag: &str) -> Result<u32, String> {
    need_value(args, index, flag)?
        .parse()
        .map_err(|_| format!("нечисловое значение {flag}"))
}

fn need_u64(args: &[String], index: &mut usize, flag: &str) -> Result<u64, String> {
    need_value(args, index, flag)?
        .parse()
        .map_err(|_| format!("нечисловое значение {flag}"))
}

fn need_i32(args: &[String], index: &mut usize, flag: &str) -> Result<i32, String> {
    need_value(args, index, flag)?
        .parse()
        .map_err(|_| format!("нечисловое значение {flag}"))
}

fn write_png(path: &std::path::Path, width: u32, height: u32) -> io::Result<()> {
    let mut raw = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    for _ in 0..height {
        raw.push(0);
        raw.extend(std::iter::repeat_n(0u8, width as usize * 4));
    }
    let mut file = fs::File::create(path)?;
    file.write_all(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'])?;
    file.write_all(&chunk(b"IHDR", &ihdr(width, height)))?;
    file.write_all(&chunk(b"IDAT", &zlib_store(&raw)))?;
    file.write_all(&chunk(b"IEND", &[]))?;
    Ok(())
}

fn ihdr(width: u32, height: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity(13);
    data.extend_from_slice(&width.to_be_bytes());
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&[8, 6, 0, 0, 0]);
    data
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + data.len());
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_src = Vec::with_capacity(4 + data.len());
    crc_src.extend_from_slice(kind);
    crc_src.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_src).to_be_bytes());
    out
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut offset = 0;
    loop {
        let end = (offset + 65535).min(data.len());
        let block = &data[offset..end];
        let last = end == data.len();
        out.push(if last { 1 } else { 0 });
        let len = block.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
        offset = end;
        if last {
            break;
        }
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
