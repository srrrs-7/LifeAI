use crate::domain::model::{DailyDataPoint, TrendResult};

/// 閾値に対する接近方向
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrossingDirection {
    /// 値が上昇して閾値を超える（コスト、エラー率）
    Rising,
    /// 値が下降して閾値を下回る（キャッシュヒット率）
    Falling,
}

/// 日次データに線形回帰を適用し、傾きと現在値を返す。
/// データポイントが3未満の場合は None を返す。
/// 日付文字列 "YYYY-MM-DD" をパースし、実日数オフセットで回帰する（欠損日対応）。
pub fn linear_regression(points: &[DailyDataPoint]) -> Option<TrendResult> {
    if points.len() < 3 {
        return None;
    }

    let base = chrono::NaiveDate::parse_from_str(&points[0].date, "%Y-%m-%d").ok()?;
    let pairs: Vec<(f64, f64)> = points
        .iter()
        .filter_map(|p| {
            let d = chrono::NaiveDate::parse_from_str(&p.date, "%Y-%m-%d").ok()?;
            let x = (d - base).num_days() as f64;
            Some((x, p.value))
        })
        .collect();

    let n = pairs.len() as f64;
    if n < 3.0 {
        return None;
    }

    let sum_x: f64 = pairs.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = pairs.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = pairs.iter().map(|(x, y)| x * y).sum();
    let sum_x2: f64 = pairs.iter().map(|(x, _)| x * x).sum();

    let denom = n * sum_x2 - sum_x * sum_x;
    if denom.abs() < f64::EPSILON {
        // x が全て同じ日 → 傾きゼロ
        return Some(TrendResult {
            slope_per_day: 0.0,
            current_value: pairs.last().unwrap().1,
            data_points: pairs.len(),
        });
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let current_value = pairs.last().unwrap().1;

    Some(TrendResult {
        slope_per_day: slope,
        current_value,
        data_points: pairs.len(),
    })
}

/// 回帰トレンドが指定方向で閾値を超えるまでの日数を返す。
/// - 既に閾値を超えている場合: None（既存の閾値ルールが対応）
/// - トレンドが逆方向の場合: None
/// - 閾値到達予測がある場合: Some(days)
pub fn days_until_crossing(
    trend: &TrendResult,
    threshold: f64,
    direction: CrossingDirection,
) -> Option<f64> {
    match direction {
        CrossingDirection::Rising => {
            if trend.current_value >= threshold {
                return None; // 既に超えている
            }
            if trend.slope_per_day <= 0.0 {
                return None; // 上昇していない
            }
            let days = (threshold - trend.current_value) / trend.slope_per_day;
            Some(days)
        }
        CrossingDirection::Falling => {
            if trend.current_value <= threshold {
                return None; // 既に下回っている
            }
            if trend.slope_per_day >= 0.0 {
                return None; // 下降していない
            }
            let days = (trend.current_value - threshold) / (-trend.slope_per_day);
            Some(days)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dp(date: &str, value: f64) -> DailyDataPoint {
        DailyDataPoint {
            date: date.to_string(),
            value,
        }
    }

    // ── linear_regression ────────────────────────────────────────

    #[test]
    fn regression_perfect_linear_ascending() {
        // y = 2 + 1*x  (day0=2, day1=3, day2=4)
        let points = vec![
            dp("2026-03-01", 2.0),
            dp("2026-03-02", 3.0),
            dp("2026-03-03", 4.0),
        ];
        let r = linear_regression(&points).unwrap();
        assert!((r.slope_per_day - 1.0).abs() < 1e-9);
        assert!((r.current_value - 4.0).abs() < 1e-9);
        assert_eq!(r.data_points, 3);
    }

    #[test]
    fn regression_perfect_linear_descending() {
        let points = vec![
            dp("2026-03-01", 10.0),
            dp("2026-03-02", 8.0),
            dp("2026-03-03", 6.0),
        ];
        let r = linear_regression(&points).unwrap();
        assert!((r.slope_per_day - (-2.0)).abs() < 1e-9);
    }

    #[test]
    fn regression_with_gaps_in_dates() {
        // day0, day2, day4 → x = [0, 2, 4], y = [1, 3, 5] → slope = 1.0
        let points = vec![
            dp("2026-03-01", 1.0),
            dp("2026-03-03", 3.0),
            dp("2026-03-05", 5.0),
        ];
        let r = linear_regression(&points).unwrap();
        assert!((r.slope_per_day - 1.0).abs() < 1e-9);
    }

    #[test]
    fn regression_flat() {
        let points = vec![
            dp("2026-03-01", 5.0),
            dp("2026-03-02", 5.0),
            dp("2026-03-03", 5.0),
        ];
        let r = linear_regression(&points).unwrap();
        assert!(r.slope_per_day.abs() < 1e-9);
    }

    #[test]
    fn regression_returns_none_for_0_points() {
        assert!(linear_regression(&[]).is_none());
    }

    #[test]
    fn regression_returns_none_for_1_point() {
        assert!(linear_regression(&[dp("2026-03-01", 5.0)]).is_none());
    }

    #[test]
    fn regression_returns_none_for_2_points() {
        let points = vec![dp("2026-03-01", 5.0), dp("2026-03-02", 6.0)];
        assert!(linear_regression(&points).is_none());
    }

    #[test]
    fn regression_all_same_date_returns_zero_slope() {
        let points = vec![
            dp("2026-03-01", 1.0),
            dp("2026-03-01", 2.0),
            dp("2026-03-01", 3.0),
        ];
        let r = linear_regression(&points).unwrap();
        assert!((r.slope_per_day - 0.0).abs() < 1e-9);
    }

    // ── days_until_crossing ──────────────────────────────────────

    #[test]
    fn crossing_rising_normal() {
        let trend = TrendResult {
            slope_per_day: 1.0,
            current_value: 8.0,
            data_points: 7,
        };
        // 8 → 10 at rate 1/day → 2 days
        let days = days_until_crossing(&trend, 10.0, CrossingDirection::Rising).unwrap();
        assert!((days - 2.0).abs() < 1e-9);
    }

    #[test]
    fn crossing_rising_already_exceeded() {
        let trend = TrendResult {
            slope_per_day: 1.0,
            current_value: 12.0,
            data_points: 7,
        };
        assert!(days_until_crossing(&trend, 10.0, CrossingDirection::Rising).is_none());
    }

    #[test]
    fn crossing_rising_wrong_direction() {
        let trend = TrendResult {
            slope_per_day: -0.5,
            current_value: 8.0,
            data_points: 7,
        };
        assert!(days_until_crossing(&trend, 10.0, CrossingDirection::Rising).is_none());
    }

    #[test]
    fn crossing_falling_normal() {
        let trend = TrendResult {
            slope_per_day: -2.0,
            current_value: 95.0,
            data_points: 7,
        };
        // 95 → 90 at rate -2/day → 2.5 days
        let days = days_until_crossing(&trend, 90.0, CrossingDirection::Falling).unwrap();
        assert!((days - 2.5).abs() < 1e-9);
    }

    #[test]
    fn crossing_falling_already_below() {
        let trend = TrendResult {
            slope_per_day: -2.0,
            current_value: 85.0,
            data_points: 7,
        };
        assert!(days_until_crossing(&trend, 90.0, CrossingDirection::Falling).is_none());
    }

    #[test]
    fn crossing_falling_wrong_direction() {
        let trend = TrendResult {
            slope_per_day: 1.0,
            current_value: 95.0,
            data_points: 7,
        };
        assert!(days_until_crossing(&trend, 90.0, CrossingDirection::Falling).is_none());
    }

    #[test]
    fn crossing_zero_slope_returns_none() {
        let trend = TrendResult {
            slope_per_day: 0.0,
            current_value: 8.0,
            data_points: 7,
        };
        assert!(days_until_crossing(&trend, 10.0, CrossingDirection::Rising).is_none());
        assert!(days_until_crossing(&trend, 5.0, CrossingDirection::Falling).is_none());
    }
}
