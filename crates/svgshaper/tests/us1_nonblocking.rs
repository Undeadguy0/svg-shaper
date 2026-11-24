mod common;

use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use svgshaper::{StopFlag, convert};

use common::{TempDir, converter_with, drive, request, write_svg};

#[test]
fn us1_poll_returns_while_converter_sleeps() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let converter = converter_with(dir.path(), &["--sleep-ms", "300"]);
    let mut req = request(vec![svg], 8, 8);
    req.set_converter(&converter);

    let started = Instant::now();
    let future = convert(req, StopFlag::new());
    let mut future = std::pin::pin!(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let polled = future.as_mut().poll(&mut context);
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "опрос ждал {:?}",
        started.elapsed()
    );
    assert!(matches!(polled, Poll::Pending));
    drive(future.as_mut()).unwrap();
}
