use canastra_data::id::ItemId;

use super::*;

/// A template document for `class` with strength `str` and one level giving `hp`.
fn template(class: u16, str: u32, hp: f64) -> (String, String) {
    let xml = format!(
        r#"<list><classId>{class}</classId><staticData>
        <baseINT>21</baseINT><baseSTR>{str}</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN>
        <baseDEX>30</baseDEX><baseWIT>11</baseWIT>
        <creationPoints><node x="-71338" y="258271" z="-3104" /></creationPoints>
        <basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><baseAtkType>FIST</baseAtkType><basePAtkSpd>300</basePAtkSpd>
        <basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
        <baseMAtk>6</baseMAtk>
        <baseMDef><rear>9</rear><lear>9</lear><rfinger>5</rfinger><lfinger>5</lfinger><neck>13</neck></baseMDef>
        <baseCanPenetrate>0</baseCanPenetrate><baseAtkRange>20</baseAtkRange>
        <baseDamRange><verticalDirection>0</verticalDirection><horizontalDirection>0</horizontalDirection><distance>26</distance><width>120</width></baseDamRange>
        <baseRndDam>10</baseRndDam>
        <baseMoveSpd><walk>80</walk><run>115</run><slowSwim>50</slowSwim><fastSwim>50</fastSwim></baseMoveSpd>
        <baseBreath>100</baseBreath><baseSafeFall>333</baseSafeFall>
        <collisionMale><radius>9</radius><height>23</height></collisionMale>
        <collisionFemale><radius>8</radius><height>23.5</height></collisionFemale>
        <newThing>1</newThing>
        </staticData><lvlUpgainData><level val="1"><hp>{hp}</hp><mp>30</mp><cp>32</cp><hpRegen>2</hpRegen><mpRegen>0.9</mpRegen><cpRegen>2</cpRegen></level></lvlUpgainData></list>"#
    );
    (format!("class{class}.xml"), xml)
}

fn migrate_with(templates: &[(String, String)]) -> (BTreeMap<ClassId, PlayerClass>, Report) {
    let class_list = r#"<list><class classId="0" name="Human Fighter" /><class classId="1" name="Warrior" parentClassId="0" /></list>"#;
    let equipment = r#"<list><equipment classId="0">
        <item id="2369" count="1" equipped="true" />
        <item id="68" count="1" minutes="2880" equipped="true" />
    </equipment></list>"#;
    let sources = Sources {
        server_class_list: class_list,
        server_class_templates: templates,
        server_initial_equipment: equipment,
        ..Sources::default()
    };
    let mut report = Report::default();
    (migrate(&sources, &mut report), report)
}

#[test]
fn starting_classes_keep_the_shared_data_once() {
    let (classes, report) = migrate_with(&[template(0, 40, 80.0), template(1, 40, 90.0)]);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(report.unmodeled.get("class template <newThing>"), Some(&2));

    let Origin::Starting(start) = &classes[&ClassId(0)].origin else { panic!("class 0 starts a line") };
    assert_eq!((start.race, start.archetype, start.sex), (Race::Human, Archetype::Fighter, None));
    assert_eq!(start.template.attributes.str, 40);
    assert_eq!(start.creation_points, [[-71_338, 258_271, -3104]]);
    let kit: Vec<_> = start.initial_items.iter().map(|item| (item.item, item.lasts_minutes)).collect();
    assert_eq!(kit, [(ItemId(2369), None), (ItemId(68), Some(2880))]);

    let warrior = &classes[&ClassId(1)];
    assert_eq!(warrior.origin, Origin::Advanced { parent: ClassId(0) });
    assert!((warrior.levels[0].hp - 90.0).abs() < f64::EPSILON);
}

#[test]
fn a_class_whose_copy_differs_is_reported() {
    let (_, report) = migrate_with(&[template(0, 40, 80.0), template(1, 41, 90.0)]);
    let messages: Vec<_> = report.diagnostics.iter().map(|d| (d.subject.as_str(), d.severity)).collect();
    assert_eq!(messages, [("class 1", Severity::Warning)]);
}
