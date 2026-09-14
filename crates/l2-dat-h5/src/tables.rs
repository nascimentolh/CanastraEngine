// Initially generated from the `L2ClientDat` (GPL) `HighFive` descriptors, then verified
// byte-for-byte against a real H5 client with `canastra scan`. Maintained by hand.

use l2_dat::schema::Kind::{Array, Group, If};
use l2_dat::schema::{ASCF, COMPACT, F32, I8, I32, Kind, Len, RGBA, Table, U8, U32, UNICODE, counter, f};

/// Mesh and texture name lists.
const MTX: Kind = Group(&[
    counter("mesh_count", I32),
    f("meshes", Array(Len::Field("mesh_count"), &[f("mesh", UNICODE)])),
    counter("texture_count", I32),
    f("textures", Array(Len::Field("texture_count"), &[f("texture", UNICODE)])),
]);

/// Like `MTX`, with two bytes per mesh and a trailing extra texture.
const MTX3: Kind = Group(&[
    counter("mesh_count", I32),
    f("meshes", Array(Len::Field("mesh_count"), &[f("mesh", UNICODE), f("val1", I8), f("val2", I8)])),
    counter("texture_count", I32),
    f("textures", Array(Len::Field("texture_count"), &[f("texture", UNICODE)])),
    f("extra_texture", UNICODE),
]);

pub(crate) const TABLES: &[Table] = &[
    ACTIONNAME,
    ADDITIONALEFFECT,
    ADDITIONALITEMGRP,
    ARMORGRP,
    CASTLENAME,
    CHARCREATEGRP,
    CHARGRP,
    CLASSINFO,
    COMMANDNAME,
    CREDITGRP,
    ENTEREVENTGRP,
    ETCITEMGRP,
    EULA,
    EXCEPTIONMINIMAPDATA,
    GAMETIP,
    GOODSICON,
    HAIRACCESSORYLOCGRP,
    HENNAGRP,
    HUNTINGZONE,
    INSTANTZONEDATA,
    ITEMNAME,
    LOGONGRP,
    MANTLEEXCEPTION,
    MOBSKILLANIMGRP,
    MUSICINFO,
    NPCGRP,
    NPCNAME,
    NPCSTRING,
    OBSCENE,
    OPTIONDATA_CLIENT,
    POSTEFFECTDATA,
    PRODUCTNAME,
    QUESTNAME,
    RAIDDATA,
    RECIPE_C,
    RIDEDATA,
    SCENEPLAYERDATA,
    SERVERNAME,
    SHORTCUTALIAS,
    SKILLGRP,
    SKILLNAME,
    SKILLSOUNDGRP,
    SKILLSOUNDSOURCE,
    STATICOBJECT,
    SYMBOLNAME,
    SYSSTRING,
    SYSTEMMSG,
    TRANSFORMDATA,
    VARIATIONEFFECTGRP,
    VEHICLEPARTSGRP,
    WEAPONGRP,
    ZONENAME,
];

const ACTIONNAME: Table = Table {
    name: "actionname",
    localized: true,
    fields: &[
        counter("data", U32),
        f("action", Array(Len::Field("data"), &[
            f("tag", U32),
            f("id", U32),
            f("type", I32),
            f("category", U32),
            counter("category2", COMPACT),
            f("category2", Array(Len::Field("category2"), &[
                f("class", I32),
            ])),
            f("cmd", ASCF),
            f("icon", ASCF),
            f("name", ASCF),
            f("desc", UNICODE),
        ])),
    ],
};

const ADDITIONALEFFECT: Table = Table {
    name: "additionaleffect",
    localized: false,
    fields: &[
        counter("data", U32),
        f("add_effect", Array(Len::Field("data"), &[
            f("id", U32),
            f("EffectNames", ASCF),
        ])),
    ],
};

const ADDITIONALITEMGRP: Table = Table {
    name: "additionalitemgrp",
    localized: false,
    fields: &[
        counter("data", U32),
        f("item", Array(Len::Field("data"), &[
            f("id", U32),
            f("has_ani", U32),
            f("unk1", U32),
            f("include_item", Group(&[
                f("unk2", U32),
                f("unk3", U32),
                f("unk4", U32),
                f("unk5", U32),
                f("unk6", U32),
                f("unk7", U32),
                f("unk9", U32),
                f("unk10", U32),
                f("unk11", U32),
                f("unk12", U32),
            ])),
            f("max_energy", U32),
        ])),
    ],
};

const ARMORGRP: Table = Table {
    name: "armorgrp",
    localized: false,
    fields: &[
        counter("data", U32),
        f("item", Array(Len::Field("data"), &[
            f("tag", U32),
            f("object_id", U32),
            f("drop_type", U32),
            f("drop_anim_type", U32),
            f("drop_radius", U32),
            f("drop_height", U32),
            f("UNK_0", U32),
            f("drop_texture", Array(Len::Fixed(1), &[
                f("drop_mesh1", UNICODE),
                f("drop_mesh2", UNICODE),
                f("drop_mesh3", UNICODE),
                f("drop_texture", Array(Len::Fixed(4), &[
                    f("drop_texture", UNICODE),
                ])),
            ])),
            f("newdata", Group(&[
                f("newdata1", U32),
                f("newdata2", U32),
                f("newdata3", U32),
                f("newdata4", U32),
                f("newdata5", U32),
                f("newdata6", U32),
                f("newdata7", U32),
                f("newdata8", U32),
            ])),
            f("icon", Group(&[
                f("icon1", UNICODE),
                f("icon2", UNICODE),
                f("icon3", UNICODE),
                f("icon4", UNICODE),
                f("icon5", UNICODE),
            ])),
            f("durability", I32),
            f("weight", U32),
            f("material_type", U32),
            f("crystallizable", U32),
            f("UNK_1", U32),
            counter("related_quest_id", U32),
            f("related_quest_id", Array(Len::Field("related_quest_id"), &[
                f("quest_id", U32),
            ])),
            f("color", U32),
            f("icon_panel", UNICODE),
            f("body_part", U32),
            f("m_HumnFigh", MTX),
            f("m_HumnFigh_add", MTX3),
            f("f_HumnFigh", MTX),
            f("f_HumnFigh_add", MTX3),
            f("m_DarkElf", MTX),
            f("m_DarkElf_add", MTX3),
            f("f_DarkElf", MTX),
            f("f_DarkElf_add", MTX3),
            f("m_Dorf", MTX),
            f("m_Dorf_add", MTX3),
            f("f_Dorf", MTX),
            f("f_Dorf_add", MTX3),
            f("m_Elf", MTX),
            f("m_Elf_add", MTX3),
            f("f_Elf", MTX),
            f("f_Elf_add", MTX3),
            f("m_HumnMyst", MTX),
            f("m_HumnMyst_add", MTX3),
            f("f_HumnMyst", MTX),
            f("f_HumnMyst_add", MTX3),
            f("m_OrcFigh", MTX),
            f("m_OrcFigh_add", MTX3),
            f("f_OrcFigh", MTX),
            f("f_OrcFigh_add", MTX3),
            f("m_OrcMage", MTX),
            f("m_OrcMage_add", MTX3),
            f("f_OrcMage", MTX),
            f("f_OrcMage_add", MTX3),
            f("m_Kamael", MTX),
            f("m_Kamael_add", MTX3),
            f("f_Kamael", MTX),
            f("f_Kamael_add", MTX3),
            f("NPC", MTX),
            f("NPC_add", MTX3),
            f("attack_effect", UNICODE),
            counter("item_sound", U32),
            f("item_sound", Array(Len::Field("item_sound"), &[
                f("item_sound_txt", UNICODE),
            ])),
            f("drop_sound", UNICODE),
            f("equip_sound", UNICODE),
            f("UNK_6", U32),
            f("UNK_7", U32),
            f("armor_type", U32),
            f("crystal_type", U32),
            f("avoid_mod", U32),
            f("pdef", U32),
            f("mdef", U32),
            f("mpbonus", U32),
            f("UNK_8", U32),
        ])),
    ],
};

const CASTLENAME: Table = Table {
    name: "castlename",
    localized: true,
    fields: &[
        counter("data", U32),
        f("castle_name", Array(Len::Field("data"), &[
            f("namber", U32),
            f("tag", U32),
            f("id", U32),
            f("name", ASCF),
            f("loc", ASCF),
            f("desc", ASCF),
            f("mark", ASCF),
            f("markgray", ASCF),
            f("flagicon", ASCF),
            f("mercname", ASCF),
        ])),
    ],
};

const CHARCREATEGRP: Table = Table {
    name: "charcreategrp",
    localized: false,
    fields: &[
        f("Char", Array(Len::Fixed(20), &[
            f("flts_1", F32),
            f("flts_2", F32),
            f("flts_3", F32),
            f("flts_4", F32),
            f("chest", U32),
            f("legs", U32),
            f("gloves", U32),
            f("feet", U32),
            f("rhand", U32),
            f("lhand", U32),
        ])),
    ],
};

const CHARGRP: Table = Table {
    name: "chargrp",
    localized: false,
    fields: &[
        f("Char", Array(Len::Fixed(17), &[
            f("hair_tab", Array(Len::Fixed(75), &[
                f("ahair_mesh0", UNICODE),
                f("ahair_texture0", UNICODE),
                f("bhair_mesh0", UNICODE),
                f("bhair_texture0", UNICODE),
            ])),
            counter("face_mesh", U32),
            f("face_mesh", Array(Len::Field("face_mesh"), &[
                f("param_face_mesh", UNICODE),
            ])),
            counter("face_texture", U32),
            f("face_texture", Array(Len::Field("face_texture"), &[
                f("param_face_texture", UNICODE),
            ])),
            f("filler_01", Array(Len::Fixed(360), &[
                f("filler_01_var", I8),
            ])),
            counter("glove_mesh", U32),
            f("glove_mesh", Array(Len::Field("glove_mesh"), &[
                f("param_glove_meshh", UNICODE),
            ])),
            counter("glove_tex", U32),
            f("glove_tex", Array(Len::Field("glove_tex"), &[
                f("param_glove_tex", UNICODE),
            ])),
            counter("glove_mesh_add", U32),
            f("glove_mesh_add", Array(Len::Field("glove_mesh_add"), &[
                f("param_glove_mesh_add", UNICODE),
            ])),
            counter("glove_tex_add", U32),
            f("glove_tex_add", Array(Len::Field("glove_tex_add"), &[
                f("param_glove_tex_add", UNICODE),
            ])),
            counter("glove_tab_byte1", COMPACT),
            f("glove_tab_byte1", Array(Len::Field("glove_tab_byte1"), &[
                f("param_glove_tab_byte1", I8),
            ])),
            counter("glove_tab_byte2", COMPACT),
            f("glove_tab_byte2", Array(Len::Field("glove_tab_byte2"), &[
                f("param_glove_tab_byte2", I8),
            ])),
            counter("upper_mesh", U32),
            f("upper_mesh", Array(Len::Field("upper_mesh"), &[
                f("param_upper_mesh", UNICODE),
            ])),
            counter("upper_tex", U32),
            f("upper_tex", Array(Len::Field("upper_tex"), &[
                f("param_upper_tex", UNICODE),
            ])),
            counter("upper_mesh_add", U32),
            f("upper_mesh_add", Array(Len::Field("upper_mesh_add"), &[
                f("param_upper_mesh_add", UNICODE),
            ])),
            counter("upper_tex_add", U32),
            f("upper_tex_add", Array(Len::Field("upper_tex_add"), &[
                f("param_upper_tex_add", UNICODE),
            ])),
            counter("upper_tab_byte1", COMPACT),
            f("upper_tab_byte1", Array(Len::Field("upper_tab_byte1"), &[
                f("param_upper_tab_byte1", I8),
            ])),
            counter("upper_tab_byte2", COMPACT),
            f("upper_tab_bytee2", Array(Len::Field("upper_tab_byte2"), &[
                f("param_upper_tab_byte2", I8),
            ])),
            counter("lower_mesh", U32),
            f("lower_mesh", Array(Len::Field("lower_mesh"), &[
                f("param_lower_mesh", UNICODE),
            ])),
            counter("lower_tex", U32),
            f("lower_tex", Array(Len::Field("lower_tex"), &[
                f("param_lower_tex", UNICODE),
            ])),
            counter("lower_mesh_add", U32),
            f("lower_mesh_add", Array(Len::Field("lower_mesh_add"), &[
                f("param_lower_mesh_add", UNICODE),
            ])),
            counter("lower_tex_add", U32),
            f("lower_tex_add", Array(Len::Field("lower_tex_add"), &[
                f("param_lower_tex_add", UNICODE),
            ])),
            counter("lower_tab_byte1", COMPACT),
            f("lower_tab_byte1", Array(Len::Field("lower_tab_byte1"), &[
                f("param_lower_tab_byte1", I8),
            ])),
            counter("lower_tab_byte2", COMPACT),
            f("lower_tab_byte2", Array(Len::Field("lower_tab_byte2"), &[
                f("param_lower_tab_byte2", I8),
            ])),
            counter("boot_mesh", U32),
            f("boot_mesh", Array(Len::Field("boot_mesh"), &[
                f("param_boot_mesh", UNICODE),
            ])),
            counter("boot_tex", U32),
            f("boot_tex", Array(Len::Field("boot_tex"), &[
                f("param_boot_tex", UNICODE),
            ])),
            counter("boot_mesh_add", U32),
            f("boot_mesh_add", Array(Len::Field("boot_mesh_add"), &[
                f("param_boot_mesh_add", UNICODE),
            ])),
            counter("boot_tex_add", U32),
            f("boot_tex_add", Array(Len::Field("boot_tex_add"), &[
                f("param_boot_tex_add", UNICODE),
            ])),
            counter("boot_tab_byte1", COMPACT),
            f("boot_tab_byte1", Array(Len::Field("boot_tab_byte1"), &[
                f("param_boot_tab_byte1", I8),
            ])),
            counter("boot_tab_byte2", COMPACT),
            f("boot_tab_byte2", Array(Len::Field("boot_tab_byte2"), &[
                f("param_boot_tab_byte2", I8),
            ])),
            f("filler_02", Array(Len::Fixed(90), &[
                f("filler_02_var", I8),
            ])),
            f("attack_effect", UNICODE),
            f("walkanimframe", U32),
            counter("attack_sound", U32),
            counter("defense_sound", U32),
            counter("damage_sound", U32),
            f("attack_sound", Array(Len::Field("attack_sound"), &[
                f("param_attack_sound", UNICODE),
            ])),
            f("defense_sound", Array(Len::Field("defense_sound"), &[
                f("param_defense_sound", UNICODE),
            ])),
            f("damage_sound", Array(Len::Field("damage_sound"), &[
                f("param_damage_sound", UNICODE),
            ])),
            counter("voice_snd_hand", U32),
            f("voice_snd_hand", Array(Len::Field("voice_snd_hand"), &[
                f("cnth", UNICODE),
            ])),
            counter("voice_snd_1hs", U32),
            f("voice_snd_1hs", Array(Len::Field("voice_snd_1hs"), &[
                f("cnt1h", UNICODE),
            ])),
            counter("voice_snd_2hs", U32),
            f("voice_snd_2hs", Array(Len::Field("voice_snd_2hs"), &[
                f("cnt2h", UNICODE),
            ])),
            counter("voice_snd_dual", U32),
            f("voice_snd_dual", Array(Len::Field("voice_snd_dual"), &[
                f("cntd", UNICODE),
            ])),
            counter("voice_snd_pole", U32),
            f("voice_snd_pole", Array(Len::Field("voice_snd_pole"), &[
                f("cntp", UNICODE),
            ])),
            counter("voice_snd_reserve1", U32),
            f("voice_snd_reserve1", Array(Len::Field("voice_snd_reserve1"), &[
                f("param_voice_snd_reserve1", UNICODE),
            ])),
            counter("voice_snd_reserve2", U32),
            f("voice_snd_reserve2", Array(Len::Field("voice_snd_reserve2"), &[
                f("param_voice_snd_reserve2", UNICODE),
            ])),
            counter("voice_snd_reserve3", U32),
            f("voice_snd_reserve3", Array(Len::Field("voice_snd_reserve3"), &[
                f("param_voice_snd_reserve3", UNICODE),
            ])),
            counter("voice_snd_reserve4", U32),
            f("voice_snd_reserve4", Array(Len::Field("voice_snd_reserve4"), &[
                f("param_voice_snd_reserve4", UNICODE),
            ])),
            counter("voice_snd_reserve5", U32),
            f("voice_snd_reserve5", Array(Len::Field("voice_snd_reserve5"), &[
                f("param_voice_snd_reserve5", UNICODE),
            ])),
            counter("voice_snd_reserve6", U32),
            f("voice_snd_reserve6", Array(Len::Field("voice_snd_reserve6"), &[
                f("param_voice_snd_reserve6", UNICODE),
            ])),
            f("final", U32),
            f("name", ASCF),
            f("unk_00", Group(&[
                f("unk_01", U32),
                f("unk_02", U32),
                f("unk_03", U32),
            ])),
            counter("p1", U32),
            f("p1", Array(Len::Field("p1"), &[
                f("cntp1", UNICODE),
            ])),
            counter("p2", U32),
            f("p2", Array(Len::Field("p2"), &[
                f("cntp2", UNICODE),
            ])),
        ])),
    ],
};

const CLASSINFO: Table = Table {
    name: "classinfo",
    localized: true,
    fields: &[
        counter("data", U32),
        f("class", Array(Len::Field("data"), &[
            f("class", U32),
            f("description", ASCF),
        ])),
    ],
};

const COMMANDNAME: Table = Table {
    name: "commandname",
    localized: true,
    fields: &[
        counter("data", U32),
        f("cmd", Array(Len::Field("data"), &[
            f("id", U32),
            f("action", U32),
            f("cmd", ASCF),
        ])),
    ],
};

const CREDITGRP: Table = Table {
    name: "creditgrp",
    localized: true,
    fields: &[
        counter("data", U32),
        f("credit", Array(Len::Field("data"), &[
            f("id", U32),
            f("html", ASCF),
            f("image", ASCF),
            f("time", U32),
            f("align", U32),
        ])),
    ],
};

const ENTEREVENTGRP: Table = Table {
    name: "entereventgrp",
    localized: false,
    fields: &[
        counter("data", U32),
        f("enterevent", Array(Len::Field("data"), &[
            f("id", U32),
            f("tag", I8),
            f("sound_name", ASCF),
            f("sound_vol", F32),
            f("sound_rad", F32),
            f("isrise", U32),
            f("spawn_type", U32),
            f("effect_name", UNICODE),
            f("anim_name", UNICODE),
        ])),
    ],
};

const ETCITEMGRP: Table = Table {
    name: "etcitemgrp",
    localized: false,
    fields: &[
        counter("data", U32),
        f("item", Array(Len::Field("data"), &[
            f("tag", U32),
            f("object_id", U32),
            f("drop_type", U32),
            f("drop_anim_type", U32),
            f("drop_radius", U32),
            f("drop_height", U32),
            f("UNK_0", U32),
            f("drop_mesh", Group(&[
                f("drop_mesh1", UNICODE),
                f("drop_mesh2", UNICODE),
                f("drop_mesh3", UNICODE),
            ])),
            f("drop_texture", Group(&[
                f("drop_tex1", UNICODE),
                f("drop_tex2", UNICODE),
                f("drop_tex3", UNICODE),
                f("drop_tex4", UNICODE),
            ])),
            f("newdata1", U32),
            f("newdata2", U32),
            f("newdata3", U32),
            f("newdata4", U32),
            f("newdata5", U32),
            f("newdata6", U32),
            f("newdata7", U32),
            f("newdata8", U32),
            f("icon", Group(&[
                f("icon1", UNICODE),
                f("icon2", UNICODE),
                f("icon3", UNICODE),
                f("icon4", UNICODE),
                f("icon5", UNICODE),
            ])),
            f("durability", U32),
            f("weight", U32),
            f("material_type", U32),
            f("crystallizable", U32),
            f("UNK_1", RGBA),
            counter("related_quest_id", U32),
            f("related_quest_id", Array(Len::Field("related_quest_id"), &[
                f("quest_id", U32),
            ])),
            f("color", U32),
            f("icon_panel", UNICODE),
            counter("mesh", I32),
            f("mesh", Array(Len::Field("mesh"), &[
                f("tab", UNICODE),
            ])),
            counter("texture", I32),
            f("texture", Array(Len::Field("texture"), &[
                f("tab2", UNICODE),
            ])),
            f("drop_sound", UNICODE),
            f("equip_sound", UNICODE),
            f("consume_type", U32),
            f("etcitem_type", U32),
            f("crystal_type", U32),
        ])),
    ],
};

const EULA: Table = Table {
    name: "eula",
    localized: true,
    fields: &[
        f("eula", Array(Len::Fixed(1), &[
            f("eula", ASCF),
            f("eulaRGB_TESTserver", ASCF),
            f("eulachinaspecialmessage", ASCF),
            f("eulachinapkagreement", ASCF),
        ])),
    ],
};

const EXCEPTIONMINIMAPDATA: Table = Table {
    name: "exceptionminimapdata",
    localized: false,
    fields: &[
        counter("data", U32),
        f("exception_location", Array(Len::Field("data"), &[
            f("location_id", U32),
            f("location_name", ASCF),
            f("max_x", U32),
            f("min_x", U32),
            f("max_y", U32),
            f("min_y", U32),
            f("max_z", U32),
            f("min_z", U32),
            f("seen_x", U32),
            f("seen_y", U32),
        ])),
    ],
};

const GAMETIP: Table = Table {
    name: "gametip",
    localized: true,
    fields: &[
        counter("data", U32),
        f("gametip", Array(Len::Field("data"), &[
            f("id", U32),
            f("priority", U32),
            f("target_lv", U32),
            f("validity", U32),
            f("tip_msg", ASCF),
        ])),
    ],
};

const GOODSICON: Table = Table {
    name: "goodsicon",
    localized: false,
    fields: &[
        counter("data", U32),
        f("goods_icon", Array(Len::Field("data"), &[
            f("id", U32),
            f("icon", UNICODE),
        ])),
    ],
};

const HAIRACCESSORYLOCGRP: Table = Table {
    name: "hairaccessorylocgrp",
    localized: false,
    fields: &[
        counter("data", U32),
        f("hairaccessory", Array(Len::Field("data"), &[
            f("mesh_name", UNICODE),
            f("X0", F32),
            f("Y0", F32),
            f("Z0", F32),
            f("Pitch0", U32),
            f("Yaw0", U32),
            f("Roll0", U32),
            f("X1", F32),
            f("Y1", F32),
            f("Z1", F32),
            f("Pitch1", U32),
            f("Yaw1", U32),
            f("Roll1", U32),
            f("X2", F32),
            f("Y2", F32),
            f("Z2", F32),
            f("Pitch2", U32),
            f("Yaw2", U32),
            f("Roll2", U32),
            f("X3", F32),
            f("Y3", F32),
            f("Z3", F32),
            f("Pitch3", U32),
            f("Yaw3", U32),
            f("Roll3", U32),
            f("X4", F32),
            f("Y4", F32),
            f("Z4", F32),
            f("Pitch4", U32),
            f("Yaw4", U32),
            f("Roll4", U32),
            f("X5", F32),
            f("Y5", F32),
            f("Z5", F32),
            f("Pitch5", U32),
            f("Yaw5", U32),
            f("Roll5", U32),
            f("X6", F32),
            f("Y6", F32),
            f("Z6", F32),
            f("Pitch6", U32),
            f("Yaw6", U32),
            f("Roll6", U32),
            f("X7", F32),
            f("Y7", F32),
            f("Z7", F32),
            f("Pitch7", U32),
            f("Yaw7", U32),
            f("Roll7", U32),
            f("X8", F32),
            f("Y8", F32),
            f("Z8", F32),
            f("Pitch8", U32),
            f("Yaw8", U32),
            f("Roll8", U32),
            f("X9", F32),
            f("Y9", F32),
            f("Z9", F32),
            f("Pitch9", U32),
            f("Yaw9", U32),
            f("Roll9", U32),
            f("X10", F32),
            f("Y10", F32),
            f("Z10", F32),
            f("Pitch10", U32),
            f("Yaw10", U32),
            f("Roll10", U32),
            f("X11", F32),
            f("Y11", F32),
            f("Z11", F32),
            f("Pitch11", U32),
            f("Yaw11", U32),
            f("Roll11", U32),
            f("X12", F32),
            f("Y12", F32),
            f("Z12", F32),
            f("Pitch12", U32),
            f("Yaw12", U32),
            f("Roll12", U32),
            f("X13", F32),
            f("Y13", F32),
            f("Z13", F32),
            f("Pitch13", U32),
            f("Yaw13", U32),
            f("Roll13", U32),
            f("X14", F32),
            f("Y14", F32),
            f("Z14", F32),
            f("Pitch14", U32),
            f("Yaw14", U32),
            f("Roll14", U32),
        ])),
    ],
};

const HENNAGRP: Table = Table {
    name: "hennagrp",
    localized: true,
    fields: &[
        counter("data", U32),
        f("symbol", Array(Len::Field("data"), &[
            f("symbol_id", U32),
            f("dye_item_id", U32),
            f("symbol_name", ASCF),
            f("symbol_icon", ASCF),
            f("symbol_add_name", ASCF),
            f("symbol_description", ASCF),
        ])),
    ],
};

const HUNTINGZONE: Table = Table {
    name: "huntingzone",
    localized: true,
    fields: &[
        counter("data", U32),
        f("Hunt", Array(Len::Field("data"), &[
            f("id", U32),
            f("type", U32),
            f("rc_level", U32),
            f("unk_1", U32),
            f("loc", Group(&[
                f("start_npc_x", F32),
                f("start_npc_y", F32),
                f("start_npc_z", F32),
            ])),
            f("desc", ASCF),
            f("search_zoneid", U32),
            f("name", ASCF),
        ])),
    ],
};

const INSTANTZONEDATA: Table = Table {
    name: "instantzonedata",
    localized: true,
    fields: &[
        counter("data", U32),
        f("InstantZoneData", Array(Len::Field("data"), &[
            f("id", U32),
            f("name", ASCF),
        ])),
    ],
};

const ITEMNAME: Table = Table {
    name: "itemname",
    localized: true,
    fields: &[
        counter("data", U32),
        f("item_name", Array(Len::Field("data"), &[
            f("id", U32),
            f("name", UNICODE),
            f("additionalname", UNICODE),
            f("description", ASCF),
            f("popup", I32),
            f("supercnt0", U32),
            counter("setid_1", U32),
            f("setid_1", Array(Len::Field("setid_1"), &[
                f("seteffect_1", UNICODE),
            ])),
            f("set_bonus_desc", ASCF),
            f("supercnt1", U32),
            counter("set_extra_id", U32),
            f("set_extra_id", Array(Len::Field("set_extra_id"), &[
                f("seteffect_2_sub2", UNICODE),
            ])),
            f("set_extra_desc", ASCF),
            f("unknown", Array(Len::Fixed(9), &[
                f("unknown_1", U8),
            ])),
            f("special_enchant_amount", U32),
            f("special_enchant_desc", ASCF),
            f("color", U32),
        ])),
    ],
};

const LOGONGRP: Table = Table {
    name: "logongrp",
    localized: false,
    fields: &[
        f("LogPawn", Array(Len::Fixed(8), &[
            f("x", F32),
            f("y", F32),
            f("z", F32),
            f("yaw", F32),
        ])),
    ],
};

const MANTLEEXCEPTION: Table = Table {
    name: "mantleexception",
    localized: false,
    fields: &[
        counter("data", U32),
        f("item", Array(Len::Field("data"), &[
            f("object_id", U32),
            counter("texture", U32),
            f("texture", Array(Len::Field("texture"), &[
                f("Goal_ID", U32),
                f("Goal_Desc", ASCF),
            ])),
        ])),
    ],
};

const MOBSKILLANIMGRP: Table = Table {
    name: "mobskillanimgrp",
    localized: false,
    fields: &[
        counter("data", U32),
        f("skill", Array(Len::Field("data"), &[
            f("npc_id", U32),
            f("skill_id", U32),
            f("seq_name", UNICODE),
            f("skill_name", ASCF),
            f("npc_name", ASCF),
            f("npc_class", ASCF),
        ])),
    ],
};

const MUSICINFO: Table = Table {
    name: "musicinfo",
    localized: false,
    fields: &[
        counter("data", U32),
        f("music", Array(Len::Field("data"), &[
            f("id", U32),
            counter("title", U32),
            f("title", Array(Len::Field("title"), &[
                f("name", UNICODE),
            ])),
        ])),
    ],
};

const NPCGRP: Table = Table {
    name: "npcgrp",
    localized: false,
    fields: &[
        counter("data", U32),
        f("npc", Array(Len::Field("data"), &[
            f("npc_id", U32),
            f("class_name", UNICODE),
            f("mesh_name", UNICODE),
            counter("texture_name", U32),
            f("texture_name", Array(Len::Field("texture_name"), &[
                f("param_texture_name", UNICODE),
            ])),
            counter("texture_name_second", U32),
            f("texture_name_second", Array(Len::Field("texture_name_second"), &[
                f("param_texture_name_second", UNICODE),
            ])),
            counter("property_list", COMPACT),
            f("property_list", Array(Len::Field("property_list"), &[
                f("param_property_list", U32),
            ])),
            f("npc_speed", F32),
            counter("unk0_cnt", U32),
            f("unk0_cnt", Array(Len::Field("unk0_cnt"), &[
                f("unk0_tab", UNICODE),
            ])),
            counter("attack_sound1", U32),
            f("attack_sound1", Array(Len::Field("attack_sound1"), &[
                f("param_attack_sound1", UNICODE),
            ])),
            counter("defense_sound1", U32),
            f("defense_sound1", Array(Len::Field("defense_sound1"), &[
                f("param_defense_sound1", UNICODE),
            ])),
            counter("damage_sound", U32),
            f("damage_sound", Array(Len::Field("damage_sound"), &[
                f("param_damage_sound", UNICODE),
            ])),
            counter("deco_effect", U32),
            f("deco_effect", Array(Len::Field("deco_effect"), &[
                f("param_deco_effect", UNICODE),
                f("param_deco_effect_scale", F32),
            ])),
            counter("quest", COMPACT),
            f("quest", Array(Len::Field("quest"), &[
                f("param_quest", U32),
            ])),
            counter("quest_step", COMPACT),
            f("quest_step", Array(Len::Field("quest_step"), &[
                f("param_quest_step", U32),
            ])),
            f("attack_effect", UNICODE),
            f("unknown_2", U32),
            f("sound_vol", F32),
            f("sound_radius", F32),
            f("sound_random", F32),
            f("social", U32),
            f("hpshowable", U32),
            counter("dialog_sound", U32),
            f("dialog_sound", Array(Len::Field("dialog_sound"), &[
                f("param_dialog_sound", ASCF),
            ])),
            f("summon_sort", U32),
        ])),
    ],
};

const NPCNAME: Table = Table {
    name: "npcname",
    localized: true,
    fields: &[
        counter("data", U32),
        f("npc", Array(Len::Field("data"), &[
            f("id", U32),
            f("name", ASCF),
            f("nick", ASCF),
            f("nickcolor", RGBA),
        ])),
    ],
};

const NPCSTRING: Table = Table {
    name: "npcstring",
    localized: true,
    fields: &[
        counter("data", U32),
        f("string", Array(Len::Field("data"), &[
            f("stringID", U32),
            f("string", ASCF),
        ])),
    ],
};

const OBSCENE: Table = Table {
    name: "obscene",
    localized: true,
    fields: &[
        counter("data", U32),
        f("obscene", Array(Len::Field("data"), &[
            f("id", U32),
            f("string", ASCF),
        ])),
    ],
};

const OPTIONDATA_CLIENT: Table = Table {
    name: "optiondata_client",
    localized: true,
    fields: &[
        counter("data", U32),
        f("option_client", Array(Len::Field("data"), &[
            f("option_id", U32),
            f("option_quality", U32),
            f("option_type", U32),
            f("option_desc1", ASCF),
            f("option_desc2", ASCF),
            f("option_desc3", ASCF),
        ])),
    ],
};

const POSTEFFECTDATA: Table = Table {
    name: "posteffectdata",
    localized: false,
    fields: &[
        counter("data", U32),
        f("posteffect_data", Array(Len::Field("data"), &[
            f("effect_id", U32),
            f("effect_name", UNICODE),
            f("effect_sort", U32),
            f("effect_play_type", U32),
            f("play_time", F32),
            f("effect_fix", U32),
            f("effect_cor1_factor1", F32),
            f("effect_cor1_factor2", F32),
            f("effect_cor1_factor3", F32),
            f("effect_cor2_factor1", F32),
            f("effect_cor2_factor2", F32),
            f("effect_cor2_factor3", F32),
            f("UNK3", U32),
            f("UNK4", U32),
        ])),
    ],
};

const PRODUCTNAME: Table = Table {
    name: "productname",
    localized: true,
    fields: &[
        counter("data", U32),
        f("product_name", Array(Len::Field("data"), &[
            f("id", U32),
            f("outer_name", UNICODE),
            f("description", ASCF),
            f("icon", UNICODE),
        ])),
    ],
};

const QUESTNAME: Table = Table {
    name: "questname",
    localized: true,
    fields: &[
        counter("data", U32),
        f("quest", Array(Len::Field("data"), &[
            f("tag", U32),
            f("id", U32),
            f("level", U32),
            f("title", ASCF),
            f("sub_name", ASCF),
            f("desc", ASCF),
            counter("goal_id", COMPACT),
            f("goal_id", Array(Len::Field("goal_id"), &[
                f("param_goal_id", U32),
            ])),
            counter("goal_num", COMPACT),
            f("goal_num", Array(Len::Field("goal_num"), &[
                f("param_goal_num", U32),
            ])),
            f("target_loc", Group(&[
                f("target_loc_x", F32),
                f("target_loc_y", F32),
                f("target_loc_z", F32),
            ])),
            f("lvl_min", U32),
            f("lvl_max", U32),
            f("quest_type", U32),
            f("entity_name", ASCF),
            f("get_item_in_quest", U32),
            f("UNK_1", U32),
            f("UNK_2", U32),
            f("start_npc_id", U32),
            f("start_npc_loc", Group(&[
                f("start_npc_loc_x", F32),
                f("start_npc_loc_y", F32),
                f("start_npc_loc_z", F32),
            ])),
            f("q_requirement", ASCF),
            f("quest_intro", ASCF),
            counter("class_limit", COMPACT),
            f("class_limit", Array(Len::Field("class_limit"), &[
                f("param_class_limit", U32),
            ])),
            counter("have_item", COMPACT),
            f("have_item", Array(Len::Field("have_item"), &[
                f("param_have_item", U32),
            ])),
            f("clan_pet_quest", U32),
            f("mark_type", U32),
            f("category_id", U32),
            f("search_zoneid", U32),
            f("iscategory", U32),
            counter("reward_id", COMPACT),
            f("reward_id", Array(Len::Field("reward_id"), &[
                f("param_reward_id", U32),
            ])),
            counter("reward_num", COMPACT),
            f("reward_num", Array(Len::Field("reward_num"), &[
                f("param_reward_num", U32),
            ])),
            counter("pre_level", COMPACT),
            f("pre_level", Array(Len::Field("pre_level"), &[
                f("param_pre_level", U32),
            ])),
        ])),
    ],
};

const RAIDDATA: Table = Table {
    name: "raiddata",
    localized: true,
    fields: &[
        counter("data", U32),
        f("raid", Array(Len::Field("data"), &[
            f("id", U32),
            f("raid_id", U32),
            f("raid_lvl", U32),
            f("search_zoneid", U32),
            f("Loc", Group(&[
                f("start_npc_x", F32),
                f("start_npc_y", F32),
                f("start_npc_z", F32),
            ])),
            f("desc", ASCF),
        ])),
    ],
};

const RECIPE_C: Table = Table {
    name: "recipe-c",
    localized: false,
    fields: &[
        counter("data", U32),
        f("recipe", Array(Len::Field("data"), &[
            f("name", ASCF),
            f("id", U32),
            f("recipe_id", U32),
            f("level", U32),
            f("product_id", U32),
            f("product_num", U32),
            f("mp_consume", U32),
            f("success_rate", U32),
            counter("material", U32),
            f("unk", U32),
            f("material", Array(Len::Field("material"), &[
                f("itemId", U32),
                f("count", U32),
            ])),
        ])),
    ],
};

/// No `L2ClientDat` descriptor for H5; derived from the client file (`Valiance` layout plus one `u32`).
/// Per-rider arrays follow race/sex order: human fighter, human mystic, elf, dark elf,
/// dwarf, orc fighter, orc mystic, kamael, each male then female.
const RIDEDATA: Table = Table {
    name: "ridedata",
    localized: false,
    fields: &[
        counter("data", U32),
        f("ride_data", Array(Len::Field("data"), &[
            f("ride_type", U32),
            f("ride_npc_id", U32),
            f("attach_bone_name", UNICODE),
            f("rider_locations", Array(Len::Fixed(16), &[f("x", F32), f("y", F32), f("z", F32)])),
            f("unk_1", Array(Len::Fixed(3), &[f("value", I32)])),
            f("rider_rotations", Array(Len::Fixed(16), &[f("pitch", I32), f("yaw", I32), f("roll", I32)])),
            f("unk_2", Array(Len::Fixed(3), &[f("value", I32)])),
            f("rider_rot", Array(Len::Fixed(16), &[f("value", F32)])),
            f("nameoffset", F32),
        ])),
    ],
};

const SCENEPLAYERDATA: Table = Table {
    name: "sceneplayerdata",
    localized: false,
    fields: &[
        counter("data", U32),
        f("sceneplayer_data", Array(Len::Field("data"), &[
            f("scene_id", U32),
            f("scene_name", UNICODE),
            f("play_time", F32),
        ])),
    ],
};

const SERVERNAME: Table = Table {
    name: "servername",
    localized: true,
    fields: &[
        counter("data", U32),
        f("server_name", Array(Len::Field("data"), &[
            f("id", U32),
            f("tag_?", U32),
            f("name", ASCF),
            f("desc", ASCF),
        ])),
    ],
};

const SHORTCUTALIAS: Table = Table {
    name: "shortcutalias",
    localized: false,
    fields: &[
        counter("data", U32),
        f("shortcutalias", Array(Len::Field("data"), &[
            f("id", U32),
            f("command", ASCF),
            f("strnum", U32),
            f("msgnum", U32),
        ])),
    ],
};

const SKILLGRP: Table = Table {
    name: "skillgrp",
    localized: false,
    fields: &[
        counter("data", U32),
        f("skill", Array(Len::Field("data"), &[
            f("skill_id", U32),
            f("skill_level", U32),
            f("icon_type", U32),
            f("operate_type", U32),
            f("mp_consume", U32),
            f("cast_range", U32),
            f("cast_style", U32),
            f("hit_time", F32),
            f("is_magic", U32),
            f("animation", UNICODE),
            f("skill_visual_effect", UNICODE),
            f("icon", UNICODE),
            f("icon_panel", UNICODE),
            f("debuff", U32),
            f("enchanted", U32),
            f("enchant_skill_level", U32),
            f("enchant_icon", ASCF),
            f("hp_consume", U32),
            f("rumble_self", U32),
            f("rumble_target", U32),
            f("GaugeTime", U32),
            f("AdditionalTag", ASCF),
        ])),
    ],
};

const SKILLNAME: Table = Table {
    name: "skillname",
    localized: true,
    fields: &[
        counter("data", U32),
        f("skill", Array(Len::Field("data"), &[
            f("skill_id", U32),
            f("skill_level", U32),
            f("name", ASCF),
            f("desc", ASCF),
            f("enchant_name", ASCF),
            f("enchant_desc", ASCF),
        ])),
    ],
};

const SKILLSOUNDGRP: Table = Table {
    name: "skillsoundgrp",
    localized: false,
    fields: &[
        counter("data", U32),
        f("skillsound", Array(Len::Field("data"), &[
            f("skill_id", U32),
            f("skill_level", U32),
            f("spelleffect_sound_1", UNICODE),
            f("spelleffect_sound_2", UNICODE),
            f("spelleffect_sound_3", UNICODE),
            f("spelleffect_sound_vol_1", F32),
            f("spelleffect_sound_rad_1", F32),
            f("spelleffect_sound_vol_2", F32),
            f("spelleffect_sound_rad_2", F32),
            f("spelleffect_sound_vol_3", F32),
            f("spelleffect_sound_rad_3", F32),
            f("shoteffect_sound_1", UNICODE),
            f("shoteffect_sound_2", UNICODE),
            f("shoteffect_sound_3", UNICODE),
            f("shoteffect_sound_vol_1", F32),
            f("shoteffect_sound_rad_1", F32),
            f("shoteffect_sound_vol_2", F32),
            f("shoteffect_sound_rad_2", F32),
            f("shoteffect_sound_vol_3", F32),
            f("shoteffect_sound_rad_3", F32),
            f("expeffect_sound_1", UNICODE),
            f("expeffect_sound_2", UNICODE),
            f("expeffect_sound_3", UNICODE),
            f("expeffect_sound_vol_1", F32),
            f("expeffect_sound_rad_1", F32),
            f("expeffect_sound_vol_2", F32),
            f("expeffect_sound_rad_2", F32),
            f("expeffect_sound_vol_3", F32),
            f("expeffect_sound_rad_3", F32),
            f("mfighter_cast", UNICODE),
            f("ffighter_cast", UNICODE),
            f("mmagic_cast", UNICODE),
            f("fmagic_cast", UNICODE),
            f("melf_cast", UNICODE),
            f("felf_cast", UNICODE),
            f("mdarkelf_cast", UNICODE),
            f("fdarkelf_cast", UNICODE),
            f("mdwarf_cast", UNICODE),
            f("fdwarf_cast", UNICODE),
            f("morc_cast", UNICODE),
            f("forc_cast", UNICODE),
            f("mshaman_cast", UNICODE),
            f("fshaman_cast", UNICODE),
            f("mkamael_cast", UNICODE),
            f("fkamael_cast", UNICODE),
            f("mextra_throw", UNICODE),
            f("mfighter_magic", UNICODE),
            f("ffighter_magic", UNICODE),
            f("mmagic_magic", UNICODE),
            f("fmagic_magic", UNICODE),
            f("melf_magic", UNICODE),
            f("felf_magic", UNICODE),
            f("mdarkelf_magic", UNICODE),
            f("fdarkelf_magic", UNICODE),
            f("mdwarf_magic", UNICODE),
            f("fdwarf_magic", UNICODE),
            f("morc_magic", UNICODE),
            f("forc_magic", UNICODE),
            f("mshaman_magic", UNICODE),
            f("fshaman_magic", UNICODE),
            f("mkamael_magic", UNICODE),
            f("fkamael_magic", UNICODE),
            f("fextra_throw", UNICODE),
            f("cast_volume", F32),
            f("cast_rad", F32),
        ])),
    ],
};

const SKILLSOUNDSOURCE: Table = Table {
    name: "skillsoundsource",
    localized: false,
    fields: &[
        counter("data", U32),
        f("skillsoundsource", Array(Len::Field("data"), &[
            f("skill_id", U32),
            f("spelleffect_sound_1_source", U32),
            f("spelleffect_sound_2_source", U32),
            f("spelleffect_sound_3_source", U32),
            f("shoteffect_sound_1_source", U32),
            f("shoteffect_sound_2_source", U32),
            f("shoteffect_sound_3_source", U32),
            f("expeffect_sound_1_source", U32),
            f("expeffect_sound_2_source", U32),
            f("expeffect_sound_3_source", U32),
        ])),
    ],
};

const STATICOBJECT: Table = Table {
    name: "staticobject",
    localized: true,
    fields: &[
        counter("data", U32),
        f("staticobject", Array(Len::Field("data"), &[
            f("id", U32),
            f("name", UNICODE),
        ])),
    ],
};

const SYMBOLNAME: Table = Table {
    name: "symbolname",
    localized: true,
    fields: &[
        counter("data", U32),
        f("symbol", Array(Len::Field("data"), &[
            f("id", U32),
            f("filename", ASCF),
            f("alias", ASCF),
            f("UNK_0", U32),
        ])),
    ],
};

const SYSSTRING: Table = Table {
    name: "sysstring",
    localized: true,
    fields: &[
        counter("data", U32),
        f("string", Array(Len::Field("data"), &[
            f("stringID", U32),
            f("string", ASCF),
        ])),
    ],
};

const SYSTEMMSG: Table = Table {
    name: "systemmsg",
    localized: true,
    fields: &[
        counter("data", U32),
        f("msg", Array(Len::Field("data"), &[
            f("id", U32),
            f("UNK_0", U32),
            f("message", ASCF),
            f("group", U32),
            f("color", RGBA),
            f("sound", ASCF),
            f("voice", ASCF),
            f("win", U32),
            f("font", U32),
            f("lftime", U32),
            f("bkg", U32),
            f("anim", U32),
            f("scrnmsg", ASCF),
            f("type", ASCF),
        ])),
    ],
};

const TRANSFORMDATA: Table = Table {
    name: "transformdata",
    localized: false,
    fields: &[
        counter("data", U32),
        f("transform_data", Array(Len::Field("data"), &[
            f("transform_id", U32),
            f("gender", U32),
            f("npc_id", U32),
            f("weapon_id", U32),
            f("transform_effect_name", UNICODE),
            f("return_effect_name", UNICODE),
            f("transform_type", U32),
            f("character_scale", F32),
            f("character_offset_x", U32),
            f("character_offset_y", U32),
        ])),
    ],
};

const VARIATIONEFFECTGRP: Table = Table {
    name: "variationeffectgrp",
    localized: true,
    fields: &[
        counter("data", U32),
        f("variation_effect", Array(Len::Field("data"), &[
            f("quality1", U32),
            f("quality2", U32),
            f("enchant_min", U32),
            f("enchant_max", U32),
            f("effect_type", U32),
            f("effect_name", UNICODE),
            f("item_prefix", ASCF),
        ])),
    ],
};

const VEHICLEPARTSGRP: Table = Table {
    name: "vehiclepartsgrp",
    localized: false,
    fields: &[
        counter("data", U32),
        f("vehicleparts", Array(Len::Field("data"), &[
            f("id", U32),
            f("actor_name", UNICODE),
            counter("partlist", COMPACT),
            f("partlist", Array(Len::Field("partlist"), &[
                f("var", I32),
            ])),
            counter("offsets", COMPACT),
            f("offsets", Array(Len::Field("offsets"), &[
                f("var0", I32),
            ])),
            f("animation", U32),
            f("serverobject", U32),
            counter("mesh", I32),
            f("mesh", Array(Len::Field("mesh"), &[
                f("mesh0", UNICODE),
            ])),
            counter("texture", I32),
            f("texture", Array(Len::Field("texture"), &[
                f("texture0", UNICODE),
            ])),
            f("spawn_sound", UNICODE),
            f("basic_sound", UNICODE),
            f("moveup_sound", UNICODE),
            f("movedown_sound", UNICODE),
            f("turn_sound", UNICODE),
            counter("random_sound", U32),
            f("random_sound", Array(Len::Field("random_sound"), &[
                f("random_sound0", UNICODE),
            ])),
        ])),
    ],
};

const WEAPONGRP: Table = Table {
    name: "weapongrp",
    localized: false,
    fields: &[
        counter("data", U32),
        f("item", Array(Len::Field("data"), &[
            f("tag", U32),
            f("object_id", U32),
            f("drop_type", U32),
            f("drop_anim_type", U32),
            f("drop_radius", U32),
            f("drop_height", U32),
            f("UNK_0", U32),
            f("drop_mesh", Group(&[
                f("drop_mesh1", UNICODE),
                f("drop_mesh2", UNICODE),
                f("drop_mesh3", UNICODE),
            ])),
            f("drop_texture", Group(&[
                f("drop_tex1", UNICODE),
                f("drop_tex2", UNICODE),
                f("drop_tex3", UNICODE),
                f("drop_tex4", UNICODE),
            ])),
            f("newdata1", U32),
            f("newdata2", U32),
            f("newdata3", U32),
            f("newdata4", U32),
            f("newdata5", U32),
            f("newdata6", U32),
            f("newdata7", U32),
            f("newdata8", U32),
            f("icon", Group(&[
                f("icon1", UNICODE),
                f("icon2", UNICODE),
                f("icon3", UNICODE),
                f("icon4", UNICODE),
                f("icon5", UNICODE),
            ])),
            f("durability", I32),
            f("weight", U32),
            f("material_type", U32),
            f("crystallizable", U32),
            f("UNK_9", U32),
            counter("related_quest_id", U32),
            f("related_quest_id", Array(Len::Field("related_quest_id"), &[
                f("quest_id", U32),
            ])),
            f("color", U32),
            f("icon_panel", UNICODE),
            f("body_part", U32),
            f("handness", U32),
            f("wp_mesh", Group(&[
                counter("wp_mesh", U32),
                f("mesh_0_", Array(Len::Field("wp_mesh"), &[
                    f("mesh0", UNICODE),
                ])),
                f("mesh_1_", Array(Len::Field("wp_mesh"), &[
                    f("mesh1", U32),
                ])),
            ])),
            counter("texture", U32),
            f("texture", Array(Len::Field("texture"), &[
                f("texture0", UNICODE),
            ])),
            counter("item_sound", U32),
            f("item_sound", Array(Len::Field("item_sound"), &[
                f("item_sound0", UNICODE),
            ])),
            f("drop_sound", UNICODE),
            f("equip_sound", UNICODE),
            f("effect", UNICODE),
            f("random_damage", U32),
            f("patt", U32),
            f("matt", U32),
            f("weapon_type", U32),
            f("crystal_type", U32),
            f("critical", U32),
            f("hit_mod", I32),
            f("avoid_mod", I32),
            f("shield_pdef", U32),
            f("shield_rate", U32),
            f("speed", U32),
            f("mp_consume", U32),
            f("soulshot_count", U32),
            f("spiritshot_count", U32),
            f("curvature", U32),
            f("UNK_10", U32),
            f("can_equip_hero", I32),
            f("UNK_12", U32),
            f("effA", UNICODE),
            f("", If { param: "wp_mesh", equals: 2, fields: &[
                f("effB", UNICODE),
            ] }),
            f("junk1A", Group(&[
                f("junk1A1", F32),
                f("junk1A2", F32),
                f("junk1A3", F32),
                f("junk1A4", F32),
                f("junk1A5", F32),
            ])),
            f("", If { param: "wp_mesh", equals: 2, fields: &[
                f("junk1B1", F32),
                f("junk1B2", F32),
                f("junk1B3", F32),
                f("junk1B4", F32),
                f("junk1B5", F32),
            ] }),
            f("rangeA", UNICODE),
            f("", If { param: "wp_mesh", equals: 2, fields: &[
                f("rangeB", UNICODE),
            ] }),
            f("junk2A", Group(&[
                f("junk2A1", F32),
                f("junk2A2", F32),
                f("junk2A3", F32),
                f("junk2A4", F32),
                f("junk2A5", F32),
                f("junk2A6", F32),
            ])),
            f("", If { param: "wp_mesh", equals: 2, fields: &[
                f("junk2B1", F32),
                f("junk2B2", F32),
                f("junk2B3", F32),
                f("junk2B4", F32),
                f("junk2B5", F32),
                f("junk2B6", F32),
            ] }),
            f("junk", Group(&[
                f("junk1", I32),
                f("junk2", I32),
                f("junk3", I32),
                f("junk4", I32),
                f("junk5", I32),
                f("junk6", I32),
            ])),
            f("variation_icon", Group(&[
                f("param_variation_icon1", UNICODE),
                f("param_variation_icon2", UNICODE),
                f("param_variation_icon3", UNICODE),
                f("param_variation_icon4", UNICODE),
            ])),
        ])),
    ],
};

const ZONENAME: Table = Table {
    name: "zonename",
    localized: true,
    fields: &[
        counter("data", U32),
        f("ZoneName", Array(Len::Field("data"), &[
            f("ID", U32),
            f("Color", U32),
            f("MapX", U32),
            f("MapY", U32),
            f("Top", F32),
            f("Bottom", F32),
            f("Name", ASCF),
            f("TownBtnLocX", U32),
            f("TownBtnLocY", U32),
            f("TownMapX", U32),
            f("TownMapY", U32),
            f("TownMapWidth", U32),
            f("TownMapHeight", U32),
            f("TownMapScale", F32),
            f("map", ASCF),
            f("dupa", U32),
        ])),
    ],
};
