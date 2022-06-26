use crate::api::logger::LogFilterInput;
use crate::db::sqlite::SQLITEPOOL;
use crate::models::sensor_logs::SensorLog;
use crate::utils::ts_to_iso8601;

use chrono::offset;
use rusqlite::params_from_iter;
use std::collections::HashMap;
use std::convert::Infallible;

pub async fn logs(params: LogFilterInput) -> Result<impl warp::Reply, Infallible> {
    let mut base_stm = String::from("SELECT * FROM sensor_logs");

    let limit = match params.limit {
        Some(limit) => limit,
        None => 10,
    };

    let mut conditions: Vec<String> = Vec::new();

    if params.sensor != None {
        conditions.push(format!("sensor = {}", params.sensor.unwrap()));
    }

    if params.after != None && params.before == None {
        conditions.push(format!(" created_at > {} ", params.after.unwrap()));
    } else if params.after == None && params.before != None {
        conditions.push(format!(" created_at < {} ", params.before.unwrap()));
    } else if params.after != None && params.before != None {
        conditions.push(format!(
            " created_at BETWEEN {} AND {} ",
            params.after.unwrap(),
            params.before.unwrap()
        ));
    }

    if conditions.len() > 0 {
        base_stm.push_str(" WHERE ");
        base_stm.push_str(&conditions.join(" AND "));
    }

    base_stm.push_str(&format!(" LIMIT {}", limit));

    println!("[Query] {}", base_stm);

    let conn = SQLITEPOOL.get().unwrap();
    let mut stmt = conn.prepare(&base_stm).unwrap();
    let results = stmt
        .query_map([], |row| {
            let timestamp: i64 = match row.get(3) {
                Ok(timestamp) => timestamp,
                Err(_) => 0,
            };

            Ok(SensorLog {
                sensor: row.get(0)?,
                outdated: row.get(1)?,
                value: row.get(2)?,
                created_at: ts_to_iso8601(timestamp / 1000),
            })
        })
        .unwrap();

    let mut sensor_logs: Vec<SensorLog> = Vec::new();
    for r in results {
        sensor_logs.push(r.unwrap())
    }

    Ok(warp::reply::json(&sensor_logs))
}

pub async fn log_saves(sensors: Vec<SensorLog>) -> Result<impl warp::Reply, Infallible> {
    let conn = SQLITEPOOL.get().unwrap();

    let mut values: Vec<String> = Vec::new();
    let mut placeholers =
        String::from("INSERT INTO sensor_logs (sensor, outdated, value, created_at) VALUES");

    for (pos, sensor) in sensors.iter().enumerate() {
        let start_pos = 4 * pos;
        placeholers = format!(
            "{}{} (?{}, ?{}, ?{}, ?{})",
            placeholers,
            if pos == 0 { "" } else { "," },
            start_pos + 1,
            start_pos + 2,
            start_pos + 3,
            start_pos + 4
        );

        values.push(sensor.sensor.to_string());
        values.push(if sensor.outdated {
            String::from("1")
        } else {
            String::from("0")
        });
        values.push(sensor.value.to_string());
        values.push(offset::Utc::now().timestamp_millis().to_string());
    }

    let result = conn.execute(&placeholers, params_from_iter(values.iter()));

    match result {
        Ok(usize) => Ok(warp::reply::json(&HashMap::from([(
            "effected_rows",
            usize,
        )]))),
        Err(err) => Ok(warp::reply::json(&HashMap::from([
            ("error", err.to_string()),
            ("sql", placeholers),
            ("values", values.concat().to_string()),
        ]))),
    }
}
