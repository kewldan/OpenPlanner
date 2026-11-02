//! Электрические сети квартиры: токи, падение напряжения, допустимые токи
//! кабелей, проверка аппаратов, состав щита по модулям DIN.
//!
//! Упрощения (как в типовых расчётах квартирных сетей): медь, скрытая прокладка,
//! cos φ по виду нагрузки, падение напряжения — по активному сопротивлению.

/// Фазное напряжение, В.
pub const PHASE_VOLTAGE: f64 = 230.0;
/// Линейное напряжение, В.
pub const LINE_VOLTAGE: f64 = 400.0;
/// Удельное сопротивление меди, Ом·мм²/м.
pub const COPPER_RESISTIVITY: f64 = 0.0175;
/// Допустимое падение напряжения от щита до потребителя, %.
pub const MAX_VOLTAGE_DROP: f64 = 5.0;

/// Ряд стандартных токов автоматов, А.
pub const BREAKERS: [f64; 9] = [6.0, 10.0, 13.0, 16.0, 20.0, 25.0, 32.0, 40.0, 50.0];
/// Ряд сечений медных жил, мм².
pub const SECTIONS: [f64; 6] = [1.5, 2.5, 4.0, 6.0, 10.0, 16.0];

/// Расчётный ток нагрузки, А: однофазной — P / (U·cos φ), трёхфазной — P / (√3·U·cos φ).
#[must_use]
pub fn current(power_w: f64, phases: u8, cos_phi: f64) -> f64 {
    let cos = cos_phi.clamp(0.1, 1.0);
    if phases == 3 {
        power_w / (3f64.sqrt() * LINE_VOLTAGE * cos)
    } else {
        power_w / (PHASE_VOLTAGE * cos)
    }
}

/// Падение напряжения в линии длиной `length_m` с сечением `section_mm2`, % от номинала.
/// Однофазная линия — прямой и обратный провод, трёхфазная — симметричная нагрузка.
#[must_use]
pub fn voltage_drop(power_w: f64, length_m: f64, section_mm2: f64, phases: u8) -> f64 {
    if section_mm2 <= 0.0 {
        return f64::INFINITY;
    }
    let r = COPPER_RESISTIVITY * length_m / section_mm2;
    if phases == 3 {
        power_w * r / (LINE_VOLTAGE * LINE_VOLTAGE) * 100.0
    } else {
        2.0 * power_w * r / (PHASE_VOLTAGE * PHASE_VOLTAGE) * 100.0
    }
}

/// Длительно допустимый ток медного кабеля при скрытой прокладке, А.
#[must_use]
pub fn ampacity(section_mm2: f64) -> f64 {
    const TABLE: [(f64, f64); 6] = [
        (1.5, 19.0),
        (2.5, 27.0),
        (4.0, 38.0),
        (6.0, 46.0),
        (10.0, 70.0),
        (16.0, 85.0),
    ];
    TABLE
        .iter()
        .rev()
        .find(|(s, _)| section_mm2 + 1e-9 >= *s)
        .map_or(0.0, |(_, a)| *a)
}

/// Наименьший стандартный автомат не меньше тока, А.
#[must_use]
pub fn breaker_for(current_a: f64) -> f64 {
    BREAKERS
        .iter()
        .copied()
        .find(|b| *b + 1e-9 >= current_a)
        .unwrap_or(BREAKERS[BREAKERS.len() - 1])
}

/// Наименьшее стандартное сечение, выдерживающее ток автомата.
#[must_use]
pub fn section_for(breaker_a: f64) -> f64 {
    SECTIONS
        .iter()
        .copied()
        .find(|s| ampacity(*s) + 1e-9 >= breaker_a)
        .unwrap_or(SECTIONS[SECTIONS.len() - 1])
}

/// Нарушение в группе.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    /// Расчётный ток больше тока автомата.
    Overload,
    /// Автомат не защищает кабель: ток автомата больше допустимого тока кабеля.
    CableUnprotected,
    /// Падение напряжения больше 5 %.
    VoltageDrop,
}

/// Расчёт группы.
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitCalc {
    /// Установленная мощность, Вт.
    pub installed: f64,
    /// Расчётная мощность (с Кс), Вт.
    pub demand: f64,
    /// Расчётный ток, А.
    pub current: f64,
    /// Падение напряжения, %.
    pub drop: f64,
    /// Допустимый ток кабеля, А.
    pub ampacity: f64,
    /// Нарушения.
    pub issues: Vec<Issue>,
}

/// Входные данные группы.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CircuitInput {
    /// Установленная мощность, Вт.
    pub installed: f64,
    /// Коэффициент спроса Кс.
    pub demand: f64,
    /// cos φ.
    pub cos_phi: f64,
    /// Фазы.
    pub phases: u8,
    /// Длина кабеля, м.
    pub length: f64,
    /// Сечение, мм².
    pub section: f64,
    /// Ток автомата, А.
    pub breaker: f64,
}

/// Расчёт группы: мощность, ток, падение напряжения, проверки.
#[must_use]
pub fn circuit(input: &CircuitInput) -> CircuitCalc {
    let demand = input.installed * input.demand;
    let i = current(demand, input.phases, input.cos_phi);
    let drop = voltage_drop(demand, input.length, input.section, input.phases);
    let cable = ampacity(input.section);
    let mut issues = Vec::new();
    if i > input.breaker + 1e-9 {
        issues.push(Issue::Overload);
    }
    if input.breaker > cable + 1e-9 {
        issues.push(Issue::CableUnprotected);
    }
    if drop > MAX_VOLTAGE_DROP {
        issues.push(Issue::VoltageDrop);
    }
    CircuitCalc {
        installed: input.installed,
        demand,
        current: i,
        drop,
        ampacity: cable,
        issues,
    }
}

/// Аппарат щита: название и ширина в модулях DIN (17,5 мм).
#[derive(Debug, Clone, PartialEq)]
pub struct PanelItem {
    /// Название: «Автомат 1P C16».
    pub name: String,
    /// Модулей на аппарат.
    pub modules: u32,
    /// Количество.
    pub count: u32,
}

/// Группа для состава щита.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelLine {
    /// Фазы.
    pub phases: u8,
    /// Ток автомата, А.
    pub breaker: f64,
    /// Дифавтомат 30 мА.
    pub rcd: bool,
}

/// Состав щита: вводной автомат, реле напряжения, групповые аппараты.
#[derive(Debug, Clone, PartialEq)]
pub struct Panel {
    /// Аппараты.
    pub items: Vec<PanelItem>,
    /// Занято модулей.
    pub modules: u32,
    /// Корпус: модулей с запасом 20 %.
    pub enclosure: u32,
    /// Ввод трёхфазный.
    pub three_phase: bool,
    /// Вводной автомат, А.
    pub main_breaker: f64,
}

/// Состав щита по группам; `total_current` — суммарный расчётный ток для вводного автомата.
#[must_use]
pub fn panel(lines: &[PanelLine], total_current: f64) -> Panel {
    let three_phase = lines.iter().any(|l| l.phases == 3);
    let mut items: Vec<PanelItem> = Vec::new();
    let mut add = |name: String, modules: u32| match items.iter_mut().find(|i| i.name == name) {
        Some(i) => i.count += 1,
        None => items.push(PanelItem {
            name,
            modules,
            count: 1,
        }),
    };
    let main_breaker = breaker_for(total_current.max(25.0));
    if three_phase {
        add(format!("Вводной автомат 3P C{main_breaker:.0}"), 3);
        add("Реле напряжения трёхфазное".into(), 3);
    } else {
        add(format!("Вводной автомат 2P C{main_breaker:.0}"), 2);
        add("Реле напряжения".into(), 2);
    }
    for l in lines {
        let (name, modules) = match (l.phases == 3, l.rcd) {
            (false, false) => (format!("Автомат 1P C{:.0}", l.breaker), 1),
            (false, true) => (format!("Дифавтомат 1P+N C{:.0} 30 мА", l.breaker), 2),
            (true, false) => (format!("Автомат 3P C{:.0}", l.breaker), 3),
            (true, true) => (format!("Дифавтомат 3P+N C{:.0} 30 мА", l.breaker), 4),
        };
        add(name, modules);
    }
    let modules: u32 = items.iter().map(|i| i.modules * i.count).sum();
    let need = modules + modules.div_ceil(5);
    let enclosure = [12, 18, 24, 36, 48, 54, 72, 96]
        .into_iter()
        .find(|e| *e >= need)
        .unwrap_or(need.div_ceil(12) * 12);
    Panel {
        items,
        modules,
        enclosure,
        three_phase,
        main_breaker,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn currents_and_drop() {
        // 3,5 кВт, cos φ = 1: 15,2 А.
        assert!((current(3500.0, 1, 1.0) - 15.217).abs() < 1e-3);
        // 7 кВт на трёх фазах: 10,1 А.
        assert!((current(7000.0, 3, 1.0) - 10.104).abs() < 1e-3);
        // 2 кВт, 20 м, 2,5 мм²: R = 0,14 Ом, ΔU = 2·2000·0,14/230² = 1,06 %.
        let drop = voltage_drop(2000.0, 20.0, 2.5, 1);
        assert!((drop - 1.0586).abs() < 1e-3, "{drop}");
    }

    #[test]
    fn cable_and_breaker_series() {
        assert!((ampacity(2.5) - 27.0).abs() < 1e-9);
        assert!((ampacity(2.0) - 19.0).abs() < 1e-9);
        assert!((breaker_for(14.2) - 16.0).abs() < 1e-9);
        assert!((section_for(16.0) - 1.5).abs() < 1e-9);
        assert!((section_for(25.0) - 2.5).abs() < 1e-9);
        assert!((section_for(32.0) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn circuit_checks() {
        let ok = circuit(&CircuitInput {
            installed: 3000.0,
            demand: 0.8,
            cos_phi: 0.9,
            phases: 1,
            length: 15.0,
            section: 2.5,
            breaker: 16.0,
        });
        assert!(ok.issues.is_empty(), "{ok:?}");
        assert!((ok.demand - 2400.0).abs() < 1e-9);
        let bad = circuit(&CircuitInput {
            installed: 7000.0,
            demand: 1.0,
            cos_phi: 1.0,
            phases: 1,
            length: 60.0,
            section: 1.5,
            breaker: 25.0,
        });
        assert_eq!(
            bad.issues,
            [Issue::Overload, Issue::CableUnprotected, Issue::VoltageDrop]
        );
    }

    #[test]
    fn panel_counts_din_modules() {
        let p = panel(
            &[
                PanelLine {
                    phases: 1,
                    breaker: 10.0,
                    rcd: false,
                },
                PanelLine {
                    phases: 1,
                    breaker: 16.0,
                    rcd: true,
                },
                PanelLine {
                    phases: 1,
                    breaker: 16.0,
                    rcd: true,
                },
            ],
            20.0,
        );
        // Ввод 2 + реле 2 + автомат 1 + два дифавтомата по 2 = 9 модулей → корпус 12.
        assert_eq!(p.modules, 9);
        assert_eq!(p.enclosure, 12);
        assert!(!p.three_phase);
        assert_eq!(
            p.items
                .iter()
                .find(|i| i.modules == 2 && i.name.starts_with("Диф"))
                .map(|i| i.count),
            Some(2)
        );
    }
}
