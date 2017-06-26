use std::str::{self, FromStr};
use std::env;
use std::time::{Instant, Duration, SystemTime, UNIX_EPOCH};
use std::process;
use log::{LogLevelFilter, LogRecord};
use env_logger::{LogBuilder};
use getopts;

use smoltcp::phy::{Tracer, FaultInjector, TapInterface};
use smoltcp::wire::EthernetFrame;
use smoltcp::wire::PrettyPrinter;

pub fn setup_logging() {
    let startup_time = Instant::now();
    LogBuilder::new()
        .format(move |record: &LogRecord| {
            let elapsed = Instant::now().duration_since(startup_time);
            let timestamp = format!("[{:6}.{:03}s]",
                                    elapsed.as_secs(), elapsed.subsec_nanos() / 1000000);
            if record.target().ends_with("::utils") {
                let mut message = format!("{}", record.args());
                message.pop();
                format!("\x1b[37m{} {}\x1b[0m", timestamp,
                        message.replace("\n", "\n             "))
            } else if record.target().starts_with("smoltcp::") {
                format!("\x1b[0m{} ({}): {}\x1b[0m", timestamp,
                        record.target().replace("smoltcp::", ""), record.args())
            } else {
                format!("\x1b[32m{} ({}): {}\x1b[0m", timestamp,
                        record.target(), record.args())
            }
        })
        .filter(None, LogLevelFilter::Trace)
        .init()
        .unwrap();
}

pub fn setup_device(more_args: &[&str])
        -> (FaultInjector<Tracer<TapInterface, EthernetFrame<&'static [u8]>>>,
            Vec<String>) {
    let mut opts = getopts::Options::new();
    opts.optopt("", "drop-chance", "Chance of dropping a packet (%)", "CHANCE");
    opts.optopt("", "corrupt-chance", "Chance of corrupting a packet (%)", "CHANCE");
    opts.optopt("", "size-limit", "Drop packets larger than given size (octets)", "SIZE");
    opts.optopt("", "tx-rate-limit", "Drop packets after transmit rate exceeds given limit \
                                      (packets per interval)", "RATE");
    opts.optopt("", "rx-rate-limit", "Drop packets after transmit rate exceeds given limit \
                                      (packets per interval)", "RATE");
    opts.optopt("", "shaping-interval", "Sets the interval for rate limiting (ms)", "RATE");
    opts.optflag("h", "help", "print this help menu");

    let matches = opts.parse(env::args().skip(1)).unwrap();
    if matches.opt_present("h") || matches.free.len() != more_args.len() + 1 {
        let brief = format!("Usage: {} INTERFACE {} [options]",
                            env::args().nth(0).unwrap(),
                            more_args.join(" "));
        print!("{}", opts.usage(&brief));
        process::exit(if matches.free.len() != more_args.len() + 1 { 1 } else { 0 });
    }
    let drop_chance    = u8::from_str(&matches.opt_str("drop-chance")
                                             .unwrap_or("0".to_string())).unwrap();
    let corrupt_chance = u8::from_str(&matches.opt_str("corrupt-chance")
                                             .unwrap_or("0".to_string())).unwrap();
    let size_limit = usize::from_str(&matches.opt_str("size-limit")
                                             .unwrap_or("0".to_string())).unwrap();
    let tx_rate_limit = u64::from_str(&matches.opt_str("tx-rate-limit")
                                              .unwrap_or("0".to_string())).unwrap();
    let rx_rate_limit = u64::from_str(&matches.opt_str("rx-rate-limit")
                                              .unwrap_or("0".to_string())).unwrap();
    let shaping_interval = u32::from_str(&matches.opt_str("shaping-interval")
                                                 .unwrap_or("0".to_string())).unwrap();

    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();

    fn trace_writer(printer: PrettyPrinter<EthernetFrame<&[u8]>>) {
        trace!("{}", printer)
    }

    let device = TapInterface::new(&matches.free[0]).unwrap();
    let device = Tracer::<_, EthernetFrame<&'static [u8]>>::new(device, trace_writer);
    let mut device = FaultInjector::new(device, seed);
    device.set_drop_chance(drop_chance);
    device.set_corrupt_chance(corrupt_chance);
    device.set_max_packet_size(size_limit);
    device.set_max_tx_rate(tx_rate_limit);
    device.set_max_rx_rate(rx_rate_limit);
    device.set_bucket_interval(Duration::from_millis(shaping_interval as u64));

    (device, matches.free[1..].to_owned())
}