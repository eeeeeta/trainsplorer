use super::*;
use types::*;

#[test]
fn test_header() {
    let rec = "A                             FILE-SPEC=05 1.00 12/01/18 18.10.25   79";
    let date = NaiveDate::from_ymd(2018, 01, 12);
    let time = NaiveTime::from_hms(18, 10, 25);
    assert_eq!(msn_header(rec).unwrap().1, MsnHeader {
        version: "05 1.00 ".into(),
        timestamp: NaiveDateTime::new(date, time),
        seq: 79
    });
}
#[test]
fn test_station() {
    let rec = "A    ABBEY WOOD                    0ABWD   ABW   ABW15473E61790 4                 ;";

    assert_eq!(msn_station(rec).unwrap().1, MsnStation {
        name: "ABBEY WOOD".into(),
        cate_type: CateType::Not,
        tiploc: "ABWD".into(),
        subsidiary_crs: "ABW".into(),
        crs: "ABW".into(),
        easting: 15473,
        estimated: true,
        northing: 61790,
        change_time: 4
    });
}
