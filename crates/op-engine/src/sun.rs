//! Положение солнца по дате, времени и месту (упрощённые формулы NOAA,
//! точность — доли градуса, для инсоляции квартиры достаточно).

use glam::DVec3;
use op_model::Site;

/// Положение солнца: азимут от севера по часовой стрелке и высота над горизонтом, градусы.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunPosition {
    /// Азимут, градусы (0 — север, 90 — восток).
    pub azimuth: f64,
    /// Высота, градусы (меньше нуля — солнце за горизонтом).
    pub altitude: f64,
}

/// Номер дня в году (1–365) для месяца 1–12 и дня месяца; год считается невисокосным.
#[must_use]
pub fn day_of_year(month: u32, day: u32) -> u32 {
    const DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let month = month.clamp(1, 12);
    let before: u32 = DAYS[..(month - 1) as usize].iter().sum();
    before + day.clamp(1, DAYS[(month - 1) as usize])
}

/// Положение солнца в день `day` (1–365) в `hour` часов местного времени пояса `site.utc_offset`.
#[must_use]
pub fn position(site: &Site, day: u32, hour: f64) -> SunPosition {
    let gamma = std::f64::consts::TAU / 365.0 * (f64::from(day) - 1.0 + (hour - 12.0) / 24.0);
    let eq_time = 229.18
        * (0.000_075 + 0.001_868 * gamma.cos()
            - 0.032_077 * gamma.sin()
            - 0.014_615 * (2.0 * gamma).cos()
            - 0.040_849 * (2.0 * gamma).sin());
    let decl = 0.006_918 - 0.399_912 * gamma.cos() + 0.070_257 * gamma.sin()
        - 0.006_758 * (2.0 * gamma).cos()
        + 0.000_907 * (2.0 * gamma).sin()
        - 0.002_697 * (3.0 * gamma).cos()
        + 0.001_48 * (3.0 * gamma).sin();
    // Истинное солнечное время, минуты, и часовой угол.
    let solar = hour * 60.0 + eq_time + 4.0 * site.longitude - 60.0 * site.utc_offset;
    let ha = (solar / 4.0 - 180.0).to_radians();
    let lat = site.latitude.to_radians();
    let cos_zenith = (lat.sin() * decl.sin() + lat.cos() * decl.cos() * ha.cos()).clamp(-1.0, 1.0);
    let altitude = 90.0 - cos_zenith.acos().to_degrees();
    // Азимут от юга (к западу положительный) → от севера по часовой.
    let from_south = ha
        .sin()
        .atan2(ha.cos() * lat.sin() - decl.tan() * lat.cos());
    let azimuth = (from_south.to_degrees() + 180.0).rem_euclid(360.0);
    SunPosition { azimuth, altitude }
}

/// Направление на солнце в координатах 3D-сцены (X — вправо по плану, Y — вверх по плану, Z — вверх).
/// Север на плане повёрнут на `site.north` градусов по часовой стрелке от верха листа.
#[must_use]
pub fn direction(site: &Site, sun: SunPosition) -> DVec3 {
    let n = site.north.to_radians();
    let north = DVec3::new(n.sin(), n.cos(), 0.0);
    let east = DVec3::new(n.cos(), -n.sin(), 0.0);
    let (az, alt) = (sun.azimuth.to_radians(), sun.altitude.to_radians());
    (east * az.sin() + north * az.cos()) * alt.cos() + DVec3::Z * alt.sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moscow() -> Site {
        Site::default()
    }

    #[test]
    fn days_are_counted_from_january() {
        assert_eq!(day_of_year(1, 1), 1);
        assert_eq!(day_of_year(3, 21), 80);
        assert_eq!(day_of_year(12, 31), 365);
        assert_eq!(day_of_year(2, 30), 59, "день ограничен длиной месяца");
    }

    #[test]
    fn noon_sun_is_south_and_high_in_summer() {
        // Москва, 21 июня: истинный полдень около 12:30 по UTC+3, высота ≈ 90 − 55,75 + 23,4.
        let noon = (0..=180)
            .map(|i| position(&moscow(), day_of_year(6, 21), 11.0 + f64::from(i) / 60.0))
            .max_by(|a, b| a.altitude.total_cmp(&b.altitude))
            .unwrap();
        assert!((noon.altitude - 57.7).abs() < 0.5, "{noon:?}");
        assert!((noon.azimuth - 180.0).abs() < 2.0, "{noon:?}");
        // Утром солнце на востоке, вечером — на западе.
        let morning = position(&moscow(), day_of_year(6, 21), 8.0);
        let evening = position(&moscow(), day_of_year(6, 21), 18.0);
        assert!((60.0..120.0).contains(&morning.azimuth), "{morning:?}");
        assert!((240.0..300.0).contains(&evening.azimuth), "{evening:?}");
    }

    #[test]
    fn winter_midnight_sun_is_below_horizon() {
        assert!(position(&moscow(), day_of_year(12, 21), 0.0).altitude < -40.0);
        let noon = position(&moscow(), day_of_year(12, 21), 12.5);
        assert!((noon.altitude - 10.8).abs() < 1.0, "{noon:?}");
    }

    #[test]
    fn direction_follows_north_on_the_plan() {
        let south_noon = SunPosition {
            azimuth: 180.0,
            altitude: 0.0,
        };
        // Север вверх листа: южное солнце светит снизу плана (−Y).
        let d = direction(&moscow(), south_noon);
        assert!((d - DVec3::new(0.0, -1.0, 0.0)).length() < 1e-9);
        // Север повёрнут на 90° по часовой (вправо листа): юг — слева.
        let site = Site {
            north: 90.0,
            ..moscow()
        };
        let d = direction(&site, south_noon);
        assert!((d - DVec3::new(-1.0, 0.0, 0.0)).length() < 1e-9, "{d}");
        // Восток при севере вверх — справа (+X).
        let east = direction(
            &moscow(),
            SunPosition {
                azimuth: 90.0,
                altitude: 0.0,
            },
        );
        assert!((east - DVec3::X).length() < 1e-9);
    }
}
