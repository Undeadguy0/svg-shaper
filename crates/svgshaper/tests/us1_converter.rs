mod common;

use svgshaper::{RequestError, StopFlag, convert};

use common::{TempDir, block_on, request, write_svg};

#[test]
fn us1_missing_converter_is_unavailable_before_write() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let dest = dir.path().join("out");
    let mut req = request(vec![svg], 8, 8);
    req.set_converter("/no/such/svgshaper-rasterizer");
    req.set_destination(&dest);
    let err = block_on(convert(req, StopFlag::new())).unwrap_err();
    assert_eq!(err, RequestError::ConverterUnavailable);
    assert!(!dest.exists());
    assert!(!dir.path().join("logo.png").exists());
}
