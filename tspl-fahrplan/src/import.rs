//! Functions for importing ITPS JSON schedule records.

use ntrod_types::schedule::{ScheduleRecord, ScheduleSegment, self};
use log::*;
use tspl_sqlite::traits::*;
use crate::types::*;
use crate::errors::Result;
use std::io::BufRead;
use tspl_sqlite::rusqlite::OptionalExtension;
use chrono::NaiveTime;
use is_sorted::IsSorted;
use failure::bail;

fn midnight_check(day_offset: &mut u8, last_time: &mut NaiveTime, time: NaiveTime) {
    if time < *last_time {
        *day_offset += 1;
    }
    *last_time = time;
}

/// Imports a single `ScheduleRecord` into the database.
///
/// Can be called with a record that's already in the database; in this case, the record will be
/// updated.
pub fn apply_schedule_record(conn: &Connection, rec: ScheduleRecord, metaseq: u32) -> Result<()> {
    match rec {
        ScheduleRecord::Delete { train_uid, schedule_start_date, stp_indicator, ..} => {
            info!("deleting schedules (UID {}, start {}, stp_indicator {:?})",
            train_uid, schedule_start_date, stp_indicator);
            conn.execute("DELETE FROM schedules
                          WHERE uid = ? AND start_date = ? AND stp_indicator = ? AND source = ?",
                          params![train_uid, schedule_start_date.naive_utc(), stp_indicator.as_char().to_string(), Schedule::SOURCE_ITPS])?;
            Ok(())
        },
        ScheduleRecord::Create {
            train_uid,
            schedule_start_date,
            schedule_end_date,
            schedule_days_runs,
            stp_indicator,
            schedule_segment,
            ..
        } => {
            use ntrod_types::schedule::LocationRecord::*;

            info!("inserting record (UID {}, start {}, stp_indicator {:?})",
            train_uid, schedule_start_date, stp_indicator);

            let ScheduleSegment {
                schedule_location,
                signalling_id,
                ..
            } = schedule_segment;

            // We *could* use an UPSERT here. (in fact, the postgres version did).
            // However, it makes the INSERT query mighty complicated,
            // and it probably isn't worth having the spaghetti SQL.
            //
            // Also because this is single-threaded, we don't need to worry about
            // racing.
            let sid: Option<i64> = conn.query_row(
                "SELECT id FROM schedules
                 WHERE uid = ? AND start_date = ? AND stp_indicator = ? AND source = ?",
                 params![train_uid, schedule_start_date.naive_utc(), stp_indicator.to_string(), Schedule::SOURCE_ITPS],
                 |row| row.get(0))
                .optional()?;
            let (sid, updated) = match sid {
                Some(s) => (s, true),
                None => {
                    let sched = Schedule {
                        id: -1,
                        tspl_id: Uuid::new_v4(),
                        uid: train_uid.clone(),
                        start_date: schedule_start_date.naive_utc(),
                        end_date: schedule_end_date.naive_utc(),
                        days: schedule_days_runs.into(),
                        stp_indicator: stp_indicator.as_char().to_string(),
                        signalling_id,
                        source: Schedule::SOURCE_ITPS,
                        file_metaseq: Some(metaseq),
                        darwin_id: None,
                        crosses_midnight: false
                    };
                    (sched.insert_self(conn)?, false)
                }
            };

            // Convert the schedule's movements into ScheduleMvts (without ids).
            let mut mvts = vec![];
            let mut day_offset = 0;
            let mut last_time = NaiveTime::from_hms(0, 0, 0);

            for loc in schedule_location {
                match loc {
                    Originating { tiploc_code, departure, platform, public_departure, .. } => {
                        // The `midnight_check` function is called every time we get a time
                        // to update the `day_offset` if we just went past midnight.
                        midnight_check(&mut day_offset, &mut last_time, departure);
                        mvts.push(ScheduleMvt {
                            tiploc: tiploc_code,
                            time: departure,
                            public_time: public_departure,
                            action: ScheduleMvt::ACTION_DEPARTURE,
                            platform,
                            day_offset,
                            ..ScheduleMvt::dummy()
                        });
                    },
                    Intermediate { tiploc_code, arrival, public_arrival, public_departure, departure, platform, .. } => {
                        midnight_check(&mut day_offset, &mut last_time, arrival);
                        mvts.push(ScheduleMvt {
                            tiploc: tiploc_code.clone(),
                            time: arrival,
                            public_time: public_arrival,
                            action: ScheduleMvt::ACTION_ARRIVAL,
                            platform: platform.clone(),
                            day_offset,
                            ..ScheduleMvt::dummy()
                        });
                        midnight_check(&mut day_offset, &mut last_time, departure);
                        mvts.push(ScheduleMvt {
                            tiploc: tiploc_code.clone(),
                            time: departure,
                            public_time: public_departure,
                            action: ScheduleMvt::ACTION_DEPARTURE,
                            platform,
                            day_offset,
                            ..ScheduleMvt::dummy()
                        });
                    },
                    Pass { tiploc_code, pass, .. } => {
                        midnight_check(&mut day_offset, &mut last_time, pass);
                        mvts.push(ScheduleMvt {
                            tiploc: tiploc_code.clone(),
                            time: pass,
                            action: ScheduleMvt::ACTION_PASS,
                            day_offset,
                            ..ScheduleMvt::dummy()
                        });
                    },
                    Terminating { tiploc_code, arrival, public_arrival, platform, .. } => {
                        midnight_check(&mut day_offset, &mut last_time, arrival);
                        mvts.push(ScheduleMvt {
                            tiploc: tiploc_code.clone(),
                            time: arrival,
                            public_time: public_arrival,
                            action: ScheduleMvt::ACTION_ARRIVAL,
                            platform,
                            day_offset,
                            ..ScheduleMvt::dummy()
                        });
                    }
                }
            }
            // (`is_sorted` is unstable, so we use a crate)
            if !IsSorted::is_sorted(&mut mvts.iter()) {
                // If the movements aren't sorted, it means:
                // - ITPS borked and gave us something in a weird order
                // - Our ordering algorithm is wrong
                //
                // Either way, yell loudly. We still want to sort the movements,
                // just so the /rest/ of the code doesn't break >_>
                // (which assumes proper sorting)
                error!("mvts not sorted! (UID {}, start {}, stp_indicator {:?})", train_uid, schedule_start_date, stp_indicator);
                error!("mvts: {:#?}", mvts);
                mvts.sort_unstable();
            }
            if updated {
                // If the record already exists, check if the movements are equal.
                // If they aren't, we just update the trainsplorer ID and replace all
                // the movements; since we've got our own DB, nobody's going to care about
                // the movements we deleted. If they are, wisely do nothing.
                //
                // (Yay, microservices!)
                info!("duplicate record (UID {}, start {}, stp_indicator {:?})", train_uid, schedule_start_date, stp_indicator);
                let orig_mvts = ScheduleMvt::from_select(conn, "WHERE parent_sched = ? ORDER BY (day_offset, time, action) ASC", &[&sid])?;
                if orig_mvts != mvts {
                    info!("replacing movements; record is new version");
                    // Ahahaha, so much simpler!
                    conn.execute("DELETE FROM schedule_movements WHERE parent_sched = ?", params![sid])?;
                    conn.execute("UPDATE schedules SET tspl_id = ? WHERE id = ?", params![Uuid::new_v4(), sid])?;
                }
                else {
                    debug!("leaving movements untouched");
                    return Ok(());
                }
            }
            for mut mvt in mvts {
                mvt.parent_sched = sid;
                mvt.insert_self(conn)?;
            }
            Ok(())
        }
    }
}

/// Imports a file containing schedule records into the database.
pub fn apply_schedule_records<R: BufRead>(conn: &mut Connection, mut rdr: R) -> Result<()> {
    let mut inserted = 0;
    let trans = conn.transaction()?;
    let mut metaseq = None;
    let mut line = String::new();
    // Reduce allocations!
    while rdr.read_line(&mut line)? > 0 {
        // (I mean, we still allocate here...)
        // FIXME(perf): could potentially use serde zero-copy here
        let rec: ::std::result::Result<schedule::Record, _> = serde_json::from_str(&line);
        let rec = match rec {
            Ok(r) => r,
            Err(e) => {
                warn!("error parsing: {}", e);
                debug!("line was: {}", line);
                continue;
            }
        };
        match rec {
            schedule::Record::Schedule(rec) => {
                if let Some(ms) = metaseq {
                    apply_schedule_record(&trans, rec, ms)?;
                    inserted += 1;
                }
                else {
                    bail!("file contained no Timetable record!");
                }
            },
            schedule::Record::Timetable(rec) => {
                info!("this is a {}-type timetable (seq {}) from {} (ts: {})",
                       rec.metadata.ty, rec.metadata.sequence, rec.owner, rec.timestamp);
                debug!("checking whether this timetable is new...");
                let files = ScheduleFile::from_select(&trans, "WHERE timestamp = ?", params![rec.timestamp])?;
                if files.len() > 0 {
                    bail!("schedule inserted already!");
                }
                let full = ScheduleFile::from_select(&trans, "WHERE sequence > ?", params![rec.metadata.sequence])?;
                if full.len() > 0 {
                    bail!("a schedule with a greater sequence number has been inserted!");
                }
                debug!("inserting file record...");
                let file = ScheduleFile {
                    timestamp: rec.timestamp,
                    sequence: rec.metadata.sequence
                };
                metaseq = Some(rec.metadata.sequence);
                file.insert_self(&trans)?;
                debug!("timetable OK");
            },
            _ => {}
        }
        line.clear();
    }
    trans.commit()?;
    info!("applied {} schedule entries", inserted);
    Ok(())
}
