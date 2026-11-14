//! The `.demo` encoding: floats and plain values come back bit for bit.

use super::*;

#[test]
fn a_float_survives_the_file_bit_for_bit() {
    for x in [0.1, 1.0 / 3.0, -0.0, f64::NAN, f64::INFINITY, 1e300, 5e-324] {
        let text = serde_json::to_string(&Exact(x)).unwrap();
        let back: Exact = serde_json::from_str(&text).unwrap();
        assert_eq!(back, Exact(x), "{x} through {text}");
    }
    assert_eq!(serde_json::to_string(&Exact(0.5)).unwrap(), "0.5");
}

#[test]
fn a_host_act_reads_back_as_written() {
    let calls = [
        HostCall::Eval("Time.SetScale(0.5)".into()),
        HostCall::Taint("an editor edit".into()),
        HostCall::Call {
            function: "Transform.SetPosition".into(),
            args: vec![
                Plain::Int(3),
                Plain::Number(1.0),
                Plain::Number(0.1),
                Plain::Nil,
                Plain::Str("x".into()),
                Plain::List(vec![Plain::Bool(true)]),
                Plain::Map(vec![("speed".into(), Plain::Number(-2.5))]),
            ],
        },
    ];
    for call in calls {
        let text = serde_json::to_string(&host_json(&call)).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(host_call(&value), Some(call), "through {text}");
    }
}
