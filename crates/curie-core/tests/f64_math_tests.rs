//! Correct-rounding checks for the dependency-free `F64Math` functions.
//!
//! Reference values were generated with `mpmath` at 80 significant digits and rounded
//! once to the nearest `f64` (subnormals rounded directly on the 2^-1074 grid). Every
//! function must reproduce them bit for bit, i.e. be correctly rounded (<= 0.5 ULP).

use curie_core::F64Math;

/// `(input bits, correctly rounded result bits)` for `sqrt`.
const SQRT: [(u64, u64); 40] = [
    (0x0D06_6E5A_74CE_C82E_u64, 0x267A_CAAB_9A4B_6EB7_u64),
    (0x1A9B_B7B2_FB50_83BB_u64, 0x2D45_0F1A_2E7F_6CA2_u64),
    (0x5CC0_22A5_21D7_CEDC_u64, 0x4E56_B910_9E4C_42B8_u64),
    (0x4D93_F24F_B706_B9CE_u64, 0x46C1_DD57_651E_1C2C_u64),
    (0x56D4_D7E2_4DAC_D61A_u64, 0x4B62_4304_446A_19D9_u64),
    (0x76A9_F330_A53B_3DA6_u64, 0x5B4C_D10F_5519_4F14_u64),
    (0x29E1_32E0_60F5_2FE6_u64, 0x34E7_75B1_BE29_4C67_u64),
    (0x5948_3046_3AAF_8C78_u64, 0x4C9B_D24B_F960_125D_u64),
    (0x348B_DFA8_CBCA_78D9_u64, 0x3A3D_DD9B_A4F6_8E49_u64),
    (0x3F84_2A85_0208_0DD6_u64, 0x3FB9_672E_4794_AF85_u64),
    (0x1F37_8BAB_E36D_6FFD_u64, 0x2F93_68D6_4CC3_E11E_u64),
    (0x00CE_509A_540C_F42A_u64, 0x205F_2562_66DF_35FE_u64),
    (0x7120_92A7_C4F6_E2F9_u64, 0x5887_0768_62C2_3418_u64),
    (0x1B57_A902_7CC9_3729_u64, 0x2DA3_74EA_2D66_8379_u64),
    (0x76B0_0EA2_3AE4_FE1D_u64, 0x5B50_074F_71EB_6BFF_u64),
    (0x259A_D5D6_2747_B44F_u64, 0x32C4_B89B_45A7_8697_u64),
    (0x35F5_9F1D_B0D5_3906_u64, 0x3AF2_997E_BB30_A0FA_u64),
    (0x4B5F_EED1_2198_3EF1_u64, 0x45A6_9A8A_5958_BE58_u64),
    (0x1F68_8E36_A591_A301_u64, 0x2FAC_081E_0CD1_B80D_u64),
    (0x463A_822E_E0C1_C2B3_u64, 0x4314_9835_F2CF_62F0_u64),
    (0x0A52_7B0D_060B_D195_u64, 0x2521_3216_B0FC_EC72_u64),
    (0x7B59_F1F3_8C57_413B_u64, 0x5DA4_5FE1_FB55_F679_u64),
    (0x203D_61F1_D6AE_F326_u64, 0x3015_AEAA_51F5_12E1_u64),
    (0x3816_9606_6341_2136_u64, 0x3C03_0289_41D3_DFF4_u64),
    (0x0853_51B2_2B09_1B79_u64, 0x2421_94D7_52ED_B822_u64),
    (0x7A31_7B30_890B_BB05_u64, 0x5D10_B966_1E7F_5A74_u64),
    (0x4984_7E70_6FEA_050B_u64, 0x44B9_9BD2_FA03_3B52_u64),
    (0x0F14_2978_6844_694C_u64, 0x2781_F5F9_D594_00E9_u64),
    (0x3F4E_3BA3_6B76_D5B8_u64, 0x3F9F_1A9B_81F7_B4AF_u64),
    (0x74CD_19E4_8957_48A0_u64, 0x5A5E_8423_AC34_B401_u64),
    (0x1724_113B_24B0_0F0A_u64, 0x2B89_573B_BD12_3750_u64),
    (0x2ABE_4E35_FA35_7BBA_u64, 0x3556_052A_4B65_4B59_u64),
    (0x283A_D7C4_BD35_2C34_u64, 0x3414_B95A_3543_8A8F_u64),
    (0x4806_4218_F874_3987_u64, 0x43FA_B030_8B30_E992_u64),
    (0x295C_0E42_D169_83FA_u64, 0x34A5_2FE2_DB73_B42B_u64),
    (0x16FC_2ECA_4D99_3C01_u64, 0x2B75_3C27_AC10_E74E_u64),
    (0x3867_9053_AFB4_38F6_u64, 0x3C2B_75B5_BF32_DD67_u64),
    (0x0E54_F9BC_F38A_AD2B_u64, 0x2722_51D2_E42E_81B1_u64),
    (0x1268_01AD_6E52_C140_u64, 0x292B_B772_D29F_D717_u64),
    (0x07C0_E5B5_296A_148B_u64, 0x23D7_40D4_BDDC_089B_u64),
];

/// `(input bits, correctly rounded result bits)` for `exp`.
const EXP: [(u64, u64); 45] = [
    (0xC02B_FB0C_A5AB_A540_u64, 0x3EAC_2C2D_403D_67FA_u64),
    (0x4060_2A37_9FB6_B114_u64, 0x4B97_B970_E456_C782_u64),
    (0x4077_4609_AAFA_6FF0_u64, 0x6182_B9DB_52A5_683D_u64),
    (0xC082_3F4A_DABF_406E_u64, 0x0B48_2555_6E9C_F4EE_u64),
    (0x406E_69D3_B8B4_75B0_u64, 0x55E0_333F_F465_E176_u64),
    (0x407A_A3B2_F9DE_5630_u64, 0x665E_4EDF_FF70_3F15_u64),
    (0x4050_7B17_485C_2E98_u64, 0x45E1_3BFC_4403_0490_u64),
    (0x4057_BE93_9EB9_9A78_u64, 0x4880_448B_AA3F_3063_u64),
    (0xC064_9CF1_2FBE_6E60_u64, 0x3111_1147_9567_FB26_u64),
    (0x4075_A3D1_0B09_8944_u64, 0x5F26_E3BB_256C_C783_u64),
    (0xC060_C268_5F60_989C_u64, 0x33D7_C1EF_B4D7_30CE_u64),
    (0xC07A_2623_86B8_1ADC_u64, 0x1A35_1C83_9B04_0EE6_u64),
    (0xC082_9E41_D25A_EEC8_u64, 0x0A36_21CA_B7E5_08F5_u64),
    (0x4080_FDFF_26B9_E486_u64, 0x70F6_151D_B76E_818B_u64),
    (0x4085_98EF_D86B_41D8_u64, 0x7E40_CF4E_AD7C_262F_u64),
    (0x407A_278D_33D7_C888_u64, 0x65AA_7DBE_6F6D_56B4_u64),
    (0xC073_CC62_06CC_A390_u64, 0x235F_D1AD_3B4E_F18A_u64),
    (0xC053_9AA8_4DC1_7130_u64, 0x38DD_3841_8E93_732A_u64),
    (0xC043_523F_E76F_4930_u64, 0x3C73_08D8_2115_1D93_u64),
    (0x4081_F666_C152_9B60_u64, 0x73C3_2D7F_65EF_F3FD_u64),
    (0x4070_1C5D_D2C5_09EE_u64, 0x572D_9A7B_8621_2EA5_u64),
    (0x4084_B0AC_F236_2D58_u64, 0x7BA2_3376_0E6A_A5BF_u64),
    (0xC033_F284_091B_F540_u64, 0x3E22_A9AA_3981_EBFC_u64),
    (0xC086_A4B1_E332_B8E8_u64, 0x0000_0000_18F8_5AAF_u64),
    (0x4079_9B2B_6A70_2A20_u64, 0x64E0_C9E6_9EA5_AA6D_u64),
    (0x406C_6DE0_BA63_7AEC_u64, 0x5471_5B64_A0AF_1217_u64),
    (0x4077_66E9_EE5E_4B9C_u64, 0x61B2_44F2_592C_ED55_u64),
    (0xC07A_4B50_5A54_E6D8_u64, 0x1A00_8A5C_0BFB_3A6B_u64),
    (0xC083_7166_6C73_2831_u64, 0x07D4_FBD2_E922_E88F_u64),
    (0xC072_BDB0_26C7_E462_u64, 0x24E5_1C83_50A7_09DC_u64),
    (0xC086_A389_F5E8_4C4A_u64, 0x0000_0000_1CDA_1674_u64),
    (0xC06C_DCF5_3B37_46F8_u64, 0x2B1D_55FC_AA99_EBD0_u64),
    (0xC072_F3FA_DE9F_295F_u64, 0x2496_B2DC_0D5F_6127_u64),
    (0x407A_9F1B_75D5_8848_u64, 0x6656_BF2D_9EE5_C864_u64),
    (0x405F_4EB9_4B44_F5E0_u64, 0x4B39_6F8D_5285_3D97_u64),
    (0x4082_9B87_ABBA_17AE_u64, 0x75A0_738A_27EE_ACC5_u64),
    (0x4076_0E04_E42C_7F34_u64, 0x5FC1_0FEF_206E_F92A_u64),
    (0x4039_E611_81C5_AAE0_u64, 0x4244_9743_1FDC_FD94_u64),
    (0x407E_34FD_9818_F6B0_u64, 0x6B83_50CF_84CA_FC0D_u64),
    (0xC072_D72D_0D10_29F4_u64, 0x24C1_2B4C_A39C_E0FE_u64),
    (0xC086_2800_0000_0000_u64, 0x0008_BFE5_5DE0_2338_u64),
    (0xC086_2400_0000_0000_u64, 0x000E_6CF6_D088_97AC_u64),
    (0x3DDB_7CDF_D9D7_BDBB_u64, 0x3FF0_0000_0006_DF38_u64),
    (0xBDDB_7CDF_D9D7_BDBB_u64, 0x3FEF_FFFF_FFF2_4190_u64),
    (0x0000_0000_0000_0000_u64, 0x3FF0_0000_0000_0000_u64),
];

/// `(input bits, correctly rounded result bits)` for `ln`.
const LN: [(u64, u64); 45] = [
    (0x4167_A98F_504D_FDD8_u64, 0x4030_556C_237A_FA25_u64),
    (0x46FA_8E81_F30C_75BE_u64, 0x4053_88E8_445C_4C11_u64),
    (0x5F34_2B6A_2D23_0BA5_u64, 0x4075_ACE2_01D4_7240_u64),
    (0x6764_76F3_A350_0D57_u64, 0x407B_59F3_A2D7_2C71_u64),
    (0x1BD5_970F_52CA_040A_u64, 0xC079_056E_492B_6742_u64),
    (0x7E2D_B691_08A0_FF98_u64, 0x4085_9267_500E_1D35_u64),
    (0x2609_644A_911B_BF8A_u64, 0xC071_F31B_9F6B_BB1E_u64),
    (0x04BA_8DEE_6C59_77AB_u64, 0xC084_84C6_7DD6_E80C_u64),
    (0x4A9C_D233_6944_F04F_u64, 0x405D_9B1B_45F2_3BB8_u64),
    (0x4C69_054B_058F_D46C_u64, 0x4061_4C44_D5CE_5364_u64),
    (0x7507_6AAD_0746_445F_u64, 0x4082_66E7_0CAB_1763_u64),
    (0x11B7_351D_2F4A_05BB_u64, 0xC080_0474_CB03_9EB3_u64),
    (0x4F2B_851E_1786_ED44_u64, 0x4065_1F44_4FC5_2A85_u64),
    (0x4FB6_39E9_2236_7ECC_u64, 0x4065_E00E_8697_D5D5_u64),
    (0x459E_F564_69AF_1D9F_u64, 0x404F_858B_16E9_F106_u64),
    (0x402B_97C9_5E88_E074_u64, 0x4004_FECB_AB81_AD1D_u64),
    (0x2329_25F9_64EE_927C_u64, 0xC073_F16B_1953_D4A3_u64),
    (0x42C8_9B58_A563_D0F9_u64, 0x403F_9F3F_3319_1396_u64),
    (0x4155_F46B_6156_F58E_u64, 0x402F_219A_B2D7_7020_u64),
    (0x6937_3B40_14FA_2732_u64, 0x407C_9D99_D5A5_26B9_u64),
    (0x6372_9978_9BE0_698C_u64, 0x4078_9DBA_F592_4905_u64),
    (0x6A97_33C1_7199_5A66_u64, 0x407D_9191_8C5C_3799_u64),
    (0x0DB0_F923_4AD5_422A_u64, 0xC081_69D9_ACC6_18EB_u64),
    (0x01AC_CD0C_7BCD_E66A_u64, 0xC085_93D6_DF91_E17B_u64),
    (0x2085_1FB9_B5BB_FB35_u64, 0xC075_C600_E08B_9D16_u64),
    (0x50CD_A876_F390_8944_u64, 0x4067_625C_0D95_2454_u64),
    (0x77FB_B4DA_5A50_6806_u64, 0x4083_6CDF_2176_569B_u64),
    (0x4AE3_41B5_03ED_51CE_u64, 0x405E_5F1B_09C9_DD6C_u64),
    (0x54EF_0B97_29CF_F2A5_u64, 0x406D_1BBF_CD74_9D31_u64),
    (0x34F9_A364_AC6A_39CC_u64, 0xC05E_616F_1D7C_86B1_u64),
    (0x0EDF_317A_ABA3_8205_u64, 0xC081_012B_26C1_EA60_u64),
    (0x6182_ACBD_7207_FBD6_u64, 0x4077_45FE_71FC_3E81_u64),
    (0x2A43_5F44_3CA1_0401_u64, 0xC06E_0A96_1DA6_73D3_u64),
    (0x14D1_BFAD_9C8C_EAB4_u64, 0xC07D_E2AF_6D10_428B_u64),
    (0x562B_619A_5F95_6E21_u64, 0x406E_D358_38D1_3A96_u64),
    (0x0748_00FE_55DA_DC01_u64, 0xC083_A23B_1E15_0313_u64),
    (0x1922_40A7_493C_A002_u64, 0xC07A_E500_BFCC_1B8F_u64),
    (0x637D_2335_2229_5838_u64, 0x4078_A4E9_9F79_8EDC_u64),
    (0x3239_EAB3_1089_202B_u64, 0xC063_0052_8D80_4EC7_u64),
    (0x2C82_5DE3_9DC4_4D1C_u64, 0xC06A_EDC9_4656_5294_u64),
    (0x3FF0_0000_0000_0001_u64, 0x3CAF_FFFF_FFFF_FFFF_u64),
    (0x3FEF_FFFF_FFFF_FFFF_u64, 0xBCA0_0000_0000_0000_u64),
    (0x3FE0_0000_0000_0000_u64, 0xBFE6_2E42_FEFA_39EF_u64),
    (0x4000_0000_0000_0000_u64, 0x3FE6_2E42_FEFA_39EF_u64),
    (0x0000_1268_8B70_E62B_u64, 0xC086_4E69_394D_9508_u64),
];

/// `(input bits, correctly rounded result bits)` for `normal_cdf`.
const NORMAL_CDF: [(u64, u64); 40] = [
    (0xC040_A3AC_99F5_49AF_u64, 0x0D9A_CC83_28BE_242D_u64),
    (0xC029_99F1_D24F_F824_u64, 0x383B_A774_7CA8_008B_u64),
    (0xC03A_F689_49BC_44CB_u64, 0x1EC6_90B0_C432_94D0_u64),
    (0xBFE0_AB58_BC10_0C00_u64, 0x3FD3_4710_E8D5_5074_u64),
    (0x401C_4DCA_8EF9_93C8_u64, 0x3FEF_FFFF_FFFF_E5E4_u64),
    (0xC041_3EA0_E04B_CA22_u64, 0x09E6_DA77_9BD5_0248_u64),
    (0x4002_8AF0_5BA9_1B30_u64, 0x3FEF_AC34_48E7_8659_u64),
    (0xC035_46ED_3A59_4D1F_u64, 0x2B29_ED16_9F41_43C0_u64),
    (0xC040_525A_C106_DFAC_u64, 0x0F7F_9DD1_2003_82A2_u64),
    (0x4010_AECF_7811_27B8_u64, 0x3FEF_FFE0_2905_0A52_u64),
    (0xC02C_585D_593D_91FC_u64, 0x368E_EC8B_F15F_B9B2_u64),
    (0xC016_5792_2DF7_4D10_u64, 0x3E49_04C3_1C6B_BAD0_u64),
    (0xC02F_A545_9CD4_9A4A_u64, 0x3450_FFD0_989D_7964_u64),
    (0xC030_6846_46A4_38C1_u64, 0x3375_CADF_C12B_F1E8_u64),
    (0xC01C_C3E9_2B0A_7F34_u64, 0x3D56_93C3_FCB1_64DD_u64),
    (0x4005_7A46_4E7D_5710_u64, 0x3FEF_E243_F845_BF85_u64),
    (0xC01F_5C9B_15B6_89E0_u64, 0x3CE4_3877_DAA4_3D63_u64),
    (0x4011_8ECB_3F98_A4C0_u64, 0x3FEF_FFF4_1585_A0C3_u64),
    (0xBFEA_AF17_001A_0E00_u64, 0x3FC9_E0E8_E39F_EA61_u64),
    (0x4010_B17F_84AC_7988_u64, 0x3FEF_FFE0_8665_966A_u64),
    (0xC031_2A67_E172_DBFC_u64, 0x3250_2DE5_3499_97CE_u64),
    (0xC041_856E_1CC2_AB23_u64, 0x082A_DEB8_191E_A1EC_u64),
    (0xC033_1467_50A8_8F5F_u64, 0x2F2C_421A_BE85_13EA_u64),
    (0x4013_8554_193C_7570_u64, 0x3FEF_FFFE_E385_98D5_u64),
    (0xC00B_513E_62DD_0570_u64, 0x3F34_ECE9_CB07_0CCD_u64),
    (0xC037_94E5_FAED_BC7E_u64, 0x267F_75AA_6BEA_D54F_u64),
    (0xC013_6EDB_696F_EB90_u64, 0x3EA3_DE35_595A_9031_u64),
    (0x4002_5D9F_8FDB_6D30_u64, 0x3FEF_A726_1170_CE40_u64),
    (0xBFFF_63C0_F004_A740_u64, 0x3F99_7CB2_A3B0_39EE_u64),
    (0xC02C_283B_52B8_76A8_u64, 0x36AD_5D2E_4CC2_F252_u64),
    (0xC041_D180_763A_52C6_u64, 0x0645_748D_C5AA_59B7_u64),
    (0xC02D_0B58_7686_9588_u64, 0x3619_A2BE_F098_6034_u64),
    (0x4016_936F_60BB_3D50_u64, 0x3FEF_FFFF_FB8A_2189_u64),
    (0xC032_0DD6_57EA_7064_u64, 0x30E4_B971_AA51_4546_u64),
    (0xC033_A386_BE8D_BA2D_u64, 0x2E31_EECA_0C1D_9E83_u64),
    (0x3FE2_FAA0_18DD_D240_u64, 0x3FE7_266D_6CD8_3256_u64),
    (0x4003_095F_E4F1_0D80_u64, 0x3FEF_B901_8E32_F7A3_u64),
    (0x4018_0318_782A_E7D8_u64, 0x3FEF_FFFF_FF7A_E7F5_u64),
    (0xC032_F3B6_C7C2_3C83_u64, 0x2F64_28D9_A1F2_052A_u64),
    (0xC02E_3877_C502_3A46_u64, 0x3550_95F9_0C1E_0018_u64),
];

/// `(k, lambda bits, correctly rounded pmf bits)`.
const POISSON: [(u32, u64, u64); 40] = [
    (
        17_071_u32,
        0x40D1_0C66_15BB_C562_u64,
        0x3F05_7397_F30E_736C_u64,
    ),
    (8_u32, 0x4010_2851_DF43_9FD5_u64, 0x3F9F_B25C_EFD9_A6E5_u64),
    (
        107_u32,
        0x4051_F65A_3380_6C17_u64,
        0x3EF7_31A1_3376_A833_u64,
    ),
    (
        24_209_u32,
        0x40D7_16A9_047C_8116_u64,
        0x3EC9_B114_4CC1_E996_u64,
    ),
    (
        34_055_u32,
        0x40E0_83F0_7B50_DCA7_u64,
        0x3F50_11CB_9009_E229_u64,
    ),
    (
        424_820_u32,
        0x4119_FFF7_1AE0_EC48_u64,
        0x3F20_6E4B_3DF1_BB4A_u64,
    ),
    (
        106_283_u32,
        0x40FA_1912_83F4_F253_u64,
        0x3F2B_6278_079F_6323_u64,
    ),
    (
        18_139_u32,
        0x40D2_09F7_46AF_4464_u64,
        0x3F22_FFA0_5EE3_3B01_u64,
    ),
    (
        61_590_u32,
        0x40EE_85B7_89D5_4C78_u64,
        0x3EBE_0C28_EA77_8979_u64,
    ),
    (0_u32, 0x401A_2952_A81D_995C_u64, 0x3F57_A87A_2D99_8E5E_u64),
    (0_u32, 0x3FC6_F9F8_B7FA_CD4A_u64, 0x3FEA_BDEE_7A2C_323D_u64),
    (
        180_u32,
        0x4069_5908_1BEB_EF7D_u64,
        0x3F80_16D8_8947_6859_u64,
    ),
    (0_u32, 0x3FB3_4F6C_CADE_1796_u64, 0x3FED_ACCD_59E9_306D_u64),
    (
        277_u32,
        0x4075_E368_0298_8C31_u64,
        0x3EDA_4362_E6ED_17F0_u64,
    ),
    (0_u32, 0x3FE0_9EB6_C3E1_BF24_u64, 0x3FE3_095C_5FED_6975_u64),
    (
        318_u32,
        0x4071_72C4_39D3_A57C_u64,
        0x3F5B_AFC7_6748_7D26_u64,
    ),
    (0_u32, 0x3FB0_976D_1C16_A18A_u64, 0x3FED_FDE9_2E9D_ECB4_u64),
    (8_u32, 0x3FCE_051E_A30F_7FEC_u64, 0x3DE8_ADFE_A4D9_F478_u64),
    (
        3010_u32,
        0x40A7_65BE_ACCA_FBF2_u64,
        0x3F7C_AB38_76C1_AFD8_u64,
    ),
    (0_u32, 0x3FD3_813A_7F0A_E2FA_u64, 0x3FE7_97F2_F8EC_C5DC_u64),
    (
        1016_u32,
        0x4091_28E8_DC66_7A5F_u64,
        0x3F41_6E43_9FC5_EC64_u64,
    ),
    (
        49_995_u32,
        0x40E8_6C4D_1FCE_7F03_u64,
        0x3F5D_12A1_12E0_A38C_u64,
    ),
    (0_u32, 0x4001_0955_053D_8046_u64, 0x3FBE_6F92_FCB9_BCD6_u64),
    (1_u32, 0x3FA0_4DE7_A8E0_0C3A_u64, 0x3F9F_962A_CA61_D0A5_u64),
    (40_u32, 0x4033_A3CE_B75A_775D_u64, 0x3EF4_2F2D_521F_3FB7_u64),
    (4_u32, 0x4034_B652_36D8_8975_u64, 0x3EE0_42AC_0AA3_6281_u64),
    (0_u32, 0x3F8B_366C_3CF4_BC05_u64, 0x3FEF_93DE_9F6E_475C_u64),
    (0_u32, 0x3FB8_AEE9_FCD5_6EE5_u64, 0x3FED_0F05_1151_9A55_u64),
    (
        2716_u32,
        0x40A7_F490_761B_F845_u64,
        0x3D9E_ADEE_D8D4_D6AD_u64,
    ),
    (0_u32, 0x4019_619F_7A25_0A38_u64, 0x3F5C_C0A5_8396_B6AA_u64),
    (
        16_848_u32,
        0x40D0_E514_99B5_FE94_u64,
        0x3EE0_8AB6_FD2D_E850_u64,
    ),
    (
        753_u32,
        0x408A_0FE2_FCC3_11C4_u64,
        0x3F30_55A6_F8DB_54C5_u64,
    ),
    (
        249_584_u32,
        0x410E_84B5_C79B_7ED9_u64,
        0x3F42_4CDA_FC12_DA2C_u64,
    ),
    (65_u32, 0x4049_234C_8BBF_25ED_u64, 0x3F7C_2CB4_43F0_5E72_u64),
    (
        683_331_u32,
        0x4124_C53F_9EE3_817F_u64,
        0x3EC1_8E81_ED61_AC84_u64,
    ),
    (
        76_814_u32,
        0x40F2_F815_7890_978F_u64,
        0x3EE3_8851_F38E_4414_u64,
    ),
    (
        2968_u32,
        0x40A6_14BF_4AF5_CEC5_u64,
        0x3F2D_422F_78CD_08FD_u64,
    ),
    (
        376_301_u32,
        0x4117_04E6_335C_04B4_u64,
        0x3F30_8B47_EDA8_8ED8_u64,
    ),
    (6_u32, 0x401C_5A70_5691_5C4C_u64, 0x3FC2_D306_3EDE_F2CA_u64),
    (0_u32, 0x3F8F_AB4D_B2FC_A7BB_u64, 0x3FEF_824C_3B7C_1113_u64),
];

#[test]
fn test_sqrt_is_correctly_rounded() {
    for &(input, expected) in &SQRT {
        let x = f64::from_bits(input);
        let got = F64Math::sqrt(x);
        assert_eq!(
            got.to_bits(),
            expected,
            "sqrt({x:e}) = {got:e}, expected {:e}",
            f64::from_bits(expected)
        );
    }
}

#[test]
fn test_exp_is_correctly_rounded() {
    for &(input, expected) in &EXP {
        let x = f64::from_bits(input);
        let got = F64Math::exp(x);
        assert_eq!(
            got.to_bits(),
            expected,
            "exp({x:e}) = {got:e}, expected {:e}",
            f64::from_bits(expected)
        );
    }
}

#[test]
fn test_ln_is_correctly_rounded() {
    for &(input, expected) in &LN {
        let x = f64::from_bits(input);
        let got = F64Math::ln(x);
        assert_eq!(
            got.to_bits(),
            expected,
            "ln({x:e}) = {got:e}, expected {:e}",
            f64::from_bits(expected)
        );
    }
}

#[test]
fn test_normal_cdf_is_correctly_rounded() {
    for &(input, expected) in &NORMAL_CDF {
        let x = f64::from_bits(input);
        let got = F64Math::normal_cdf(x);
        assert_eq!(
            got.to_bits(),
            expected,
            "normal_cdf({x:e}) = {got:e}, expected {:e}",
            f64::from_bits(expected)
        );
    }
}

#[test]
fn test_poisson_pmf_is_correctly_rounded() {
    for &(k, lambda_bits, expected) in &POISSON {
        let lambda = f64::from_bits(lambda_bits);
        let got = F64Math::poisson_pmf(k, lambda);
        assert_eq!(
            got.to_bits(),
            expected,
            "pmf({k}, {lambda:e}) = {got:e}, expected {:e}",
            f64::from_bits(expected)
        );
    }
}

#[test]
fn test_special_values() {
    assert!(
        F64Math::sqrt(-1.0_f64).is_nan(),
        "sqrt of a negative number is NaN"
    );
    assert!(
        F64Math::ln(-1.0_f64).is_nan(),
        "ln of a negative number is NaN"
    );
    assert_eq!(
        F64Math::ln(0.0_f64).to_bits(),
        f64::NEG_INFINITY.to_bits(),
        "ln(0) = -inf"
    );
    assert_eq!(
        F64Math::exp(710.0_f64).to_bits(),
        f64::INFINITY.to_bits(),
        "exp overflows to +inf"
    );
    assert_eq!(
        F64Math::exp(-746.0_f64).to_bits(),
        0_u64,
        "exp underflows to +0"
    );
    assert_eq!(
        F64Math::normal_cdf(50.0_f64).to_bits(),
        1.0_f64.to_bits(),
        "normal_cdf(+inf side) = 1"
    );
    assert_eq!(
        F64Math::normal_cdf(-50.0_f64).to_bits(),
        0_u64,
        "normal_cdf(-inf side) = 0"
    );
    assert_eq!(
        F64Math::poisson_pmf(0_u32, 0.0_f64).to_bits(),
        1.0_f64.to_bits(),
        "P(0; 0) = 1"
    );
    assert_eq!(
        F64Math::poisson_pmf(3_u32, 0.0_f64).to_bits(),
        0_u64,
        "P(3; 0) = 0"
    );
}
