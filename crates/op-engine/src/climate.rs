//! Климат: зоны тёплого пола (площадь обогрева и мощность), воздухообмен
//! помещений по установленным бризерам, вентиляторам и решёткам, рекомендуемая
//! мощность кондиционеров.

use std::collections::HashSet;

use glam::DVec2;
use op_calc::climate as calc;
use op_model::{DeviceKind, Document};

use crate::{
    derived::Derived,
    protocol::{ClimateReport, HeatingInfo, RoomClimate},
};

/// Площадь мебели на полу в помещении (предметы ниже 50 мм над полом), м².
fn furniture_area(doc: &Document, derived: &Derived, region: usize) -> f64 {
    doc.furniture
        .iter()
        .filter(|f| derived.visible.contains(&f.layer) && f.elevation < 50.0)
        .filter(|f| derived.regions[region].contains(DVec2::new(f.x, f.y)))
        .map(|f| f.width * f.depth / 1e6)
        .sum()
}

/// Зоны тёплого пола вида с площадью обогрева и мощностью.
#[must_use]
pub fn zones(doc: &Document, derived: &Derived) -> Vec<HeatingInfo> {
    doc.heating
        .iter()
        .filter_map(|z| {
            let room = doc.room(&z.room)?;
            let i = *derived.room_regions.get(&z.room)?;
            let region = &derived.regions[i];
            let area = op_geom::mm2_to_m2(region.area);
            let heated = calc::heated_area(
                area,
                region.perimeter / 1000.0,
                z.margin,
                furniture_area(doc, derived, i),
            );
            Some(HeatingInfo {
                id: z.id.clone(),
                room: z.room.clone(),
                room_name: room.name.clone(),
                area: (area * 100.0).round() / 100.0,
                heated,
                power: (heated * z.density).round(),
                thermostat: z.thermostat.clone(),
            })
        })
        .collect()
}

/// Климат вида.
#[must_use]
pub fn report(doc: &Document, derived: &Derived) -> ClimateReport {
    let zones = zones(doc, derived);
    let mut used = HashSet::new();
    let devices: Vec<&op_model::Device> = doc
        .devices
        .iter()
        .filter(|d| derived.visible.contains(&d.layer) && d.kind.is_climate())
        .collect();
    let mut warnings = Vec::new();
    let rooms: Vec<RoomClimate> = doc
        .rooms
        .iter()
        .filter(|r| derived.visible.contains(&r.layer))
        .filter_map(|r| {
            let i = *derived.room_regions.get(&r.id)?;
            used.insert(i).then_some((r, i))
        })
        .map(|(r, i)| {
            let region = &derived.regions[i];
            let area = op_geom::mm2_to_m2(region.area);
            let inside: Vec<&&op_model::Device> = devices
                .iter()
                .filter(|d| region.contains(DVec2::new(d.x, d.y)))
                .collect();
            let air_provided = inside
                .iter()
                .map(|d| match d.kind {
                    DeviceKind::Breather => calc::BREATHER_AIR,
                    DeviceKind::ExhaustFan => calc::FAN_AIR,
                    DeviceKind::VentGrille => calc::GRILLE_AIR,
                    _ => 0.0,
                })
                .sum();
            let ac_units = inside
                .iter()
                .filter(|d| d.kind == DeviceKind::AcIndoor)
                .count();
            let ceiling = r.ceiling_height.unwrap_or(doc.default_ceiling_height);
            let room = RoomClimate {
                id: r.id.clone(),
                name: r.name.clone(),
                area: (area * 100.0).round() / 100.0,
                air_required: calc::air_required(&r.name, area),
                air_provided,
                cooling: calc::cooling_kw(area, ceiling),
                ac_units: u32::try_from(ac_units).unwrap_or(u32::MAX),
            };
            // Замечание — только если в помещении уже что-то стоит: значит, вентиляцию проектируют.
            if !inside.is_empty() && room.air_provided + 1e-9 < room.air_required {
                warnings.push(format!(
                    "{}: воздухообмен {:.0} м³/ч меньше нужного {:.0} м³/ч",
                    room.name, room.air_provided, room.air_required
                ));
            }
            room
        })
        .collect();
    for z in &zones {
        match &z.thermostat {
            None => warnings.push(format!(
                "{}: у тёплого пола нет терморегулятора",
                z.room_name
            )),
            Some(t)
                if doc
                    .device(t)
                    .is_none_or(|d| d.kind != DeviceKind::Thermostat) =>
            {
                warnings.push(format!(
                    "{}: терморегулятор тёплого пола не найден",
                    z.room_name
                ));
            }
            Some(_) => {}
        }
        if z.heated <= 0.0 {
            warnings.push(format!(
                "{}: под тёплый пол не осталось площади",
                z.room_name
            ));
        }
    }
    ClimateReport {
        zones,
        rooms,
        warnings,
    }
}
