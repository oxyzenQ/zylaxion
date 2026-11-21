// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

// GENERATED FILE -- do not edit by hand. Regenerate with:
//   python3 scripts/engine/generate-detmath.py
// night-frontier-Z5: the certified bounded-domain transcendental
// tables. Coefficients are exact rationals (1/k! and friends)
// converted to f64 by correctly-rounded division; every bound below
// is an exact-Fraction certificate rounded OUTWARD (an over-bound,
// never an under-bound). The companion vectors live in
// test/engine/detmath_vectors.rs and pin the bits of the lane.

// The golden bit vectors: (x_bits, y_bits) pairs the test tree
// pins bit for bit, plus the two-input det_pow table. Produced
// by the generator's own simulation of the algorithm (same
// IEEE-754 ops, same order, same round-half-away).

/// One single-input golden vector: the input's bit pattern and
/// the exact bits the lane must return.
pub struct DetVector {
    pub name: &'static str,
    pub xb: u64,
    pub yb: u64,
}

/// One det_pow golden vector: base bits, exponent bits, result bits.
pub struct PowVector {
    pub cb: u64,
    pub rb: u64,
    pub yb: u64,
}

pub(crate) const VECTORS: &[DetVector] = &[
    DetVector {
        name: "exp(0.0)",
        xb: 0x0000000000000000,
        yb: 0x3ff0000000000000,
    },
    DetVector {
        name: "exp(-1e-12)",
        xb: 0xbd719799812dea11,
        yb: 0x3fefffffffffdcd1,
    },
    DetVector {
        name: "exp(-0.001)",
        xb: 0xbf50624dd2f1a9fc,
        yb: 0x3feff7cfe56f1a9e,
    },
    DetVector {
        name: "exp(-0.005208333)",
        xb: 0xbf7555553e6d4575,
        yb: 0x3fefd571baa9e224,
    },
    DetVector {
        name: "exp(-0.0226757)",
        xb: 0xbf97384c77a7d74a,
        yb: 0x3fef4854b8fd6cda,
    },
    DetVector {
        name: "exp(-0.0231)",
        xb: 0xbf97a786c226809d,
        yb: 0x3fef44ef092b7d7a,
    },
    DetVector {
        name: "exp(-0.125)",
        xb: 0xbfc0000000000000,
        yb: 0x3fec3d6a24ed8222,
    },
    DetVector {
        name: "exp(-0.3466)",
        xb: 0xbfd62eb1c432ca58,
        yb: 0x3fe6a0773cd678a3,
    },
    DetVector {
        name: "exp(-0.35)",
        xb: 0xbfd6666666666666,
        yb: 0x3fe68cce09671f71,
    },
    DetVector {
        name: "exp(-1.0)",
        xb: 0xbff0000000000000,
        yb: 0x3fd78b56362cef38,
    },
    DetVector {
        name: "exp(-3.0)",
        xb: 0xc008000000000000,
        yb: 0x3fa97db0ccceb0af,
    },
    DetVector {
        name: "exp(-2.9999999)",
        xb: 0xc007fffff29406b3,
        yb: 0x3fa97db0f792fb9a,
    },
    DetVector {
        name: "exp(-8.0)",
        xb: 0xc020000000000000,
        yb: 0x3f35fc21041027ad,
    },
    DetVector {
        name: "exp(-32.0)",
        xb: 0xc040000000000000,
        yb: 0x3d0c8464f7616468,
    },
    DetVector {
        name: "exp(-63.999)",
        xb: 0xc04fffdf3b645a1d,
        yb: 0x3a297056c67421d4,
    },
    DetVector {
        name: "exp(-64.0)",
        xb: 0xc050000000000000,
        yb: 0x3a2969d47321e4cc,
    },
    DetVector {
        name: "ln(1.0)",
        xb: 0x3ff0000000000000,
        yb: 0x0000000000000000,
    },
    DetVector {
        name: "ln(0.9999)",
        xb: 0x3fefff2e48e8a71e,
        yb: 0xbf1a3738d2cf1cc2,
    },
    DetVector {
        name: "ln(0.9994)",
        xb: 0x3feffb15b573eab3,
        yb: 0xbf43aaace3fac4b0,
    },
    DetVector {
        name: "ln(0.995)",
        xb: 0x3fefd70a3d70a3d7,
        yb: 0xbf748807f33b3512,
    },
    DetVector {
        name: "ln(0.99)",
        xb: 0x3fefae147ae147ae,
        yb: 0xbf8495453e6fd4bc,
    },
    DetVector {
        name: "ln(0.5)",
        xb: 0x3fe0000000000000,
        yb: 0xbfe62e42fefa39ef,
    },
    DetVector {
        name: "ln(0.47)",
        xb: 0x3fde147ae147ae14,
        yb: 0xbfe829251f66e5d3,
    },
    DetVector {
        name: "ln(0.04)",
        xb: 0x3fa47ae147ae147b,
        yb: 0xc009c041f7ed8d33,
    },
    DetVector {
        name: "ln(0.7071067811865476)",
        xb: 0x3fe6a09e667f3bcd,
        yb: 0xbfd62e42fefa39ee,
    },
    DetVector {
        name: "ln(0.7071067811865475)",
        xb: 0x3fe6a09e667f3bcc,
        yb: 0xbfd62e42fefa39f1,
    },
    DetVector {
        name: "ln(0.7071067811865476)",
        xb: 0x3fe6a09e667f3bcd,
        yb: 0xbfd62e42fefa39ee,
    },
    DetVector {
        name: "ln(0.3535533905932738)",
        xb: 0x3fd6a09e667f3bcd,
        yb: 0xbff0a2b23f3bab73,
    },
    DetVector {
        name: "ln(1e-08)",
        xb: 0x3e45798ee2308c3a,
        yb: 0xc0326bb1bbb55516,
    },
    DetVector {
        name: "ln(9.313225746154785e-10)",
        xb: 0x3e10000000000000,
        yb: 0xc034cb5ecf0a9650,
    },
    DetVector {
        name: "sin(0.0)",
        xb: 0x0000000000000000,
        yb: 0x0000000000000000,
    },
    DetVector {
        name: "cos(0.0)",
        xb: 0x0000000000000000,
        yb: 0x3ff0000000000000,
    },
    DetVector {
        name: "sin(1e-12)",
        xb: 0x3d719799812dea11,
        yb: 0x3d719799812dea11,
    },
    DetVector {
        name: "cos(1e-12)",
        xb: 0x3d719799812dea11,
        yb: 0x3ff0000000000000,
    },
    DetVector {
        name: "sin(1e-06)",
        xb: 0x3eb0c6f7a0b5ed8d,
        yb: 0x3eb0c6f7a0b5ea7a,
    },
    DetVector {
        name: "cos(1e-06)",
        xb: 0x3eb0c6f7a0b5ed8d,
        yb: 0x3fefffffffffee68,
    },
    DetVector {
        name: "sin(0.1)",
        xb: 0x3fb999999999999a,
        yb: 0x3fb98eaecb8bcb2c,
    },
    DetVector {
        name: "cos(0.1)",
        xb: 0x3fb999999999999a,
        yb: 0x3fefd712f9a817c0,
    },
    DetVector {
        name: "sin(0.2849)",
        xb: 0x3fd23bcd35a85879,
        yb: 0x3fd1fce94e30bd0b,
    },
    DetVector {
        name: "cos(0.2849)",
        xb: 0x3fd23bcd35a85879,
        yb: 0x3feeb5c74e9f8c89,
    },
    DetVector {
        name: "sin(0.7853981633974483)",
        xb: 0x3fe921fb54442d18,
        yb: 0x3fe6a09e667f3bcc,
    },
    DetVector {
        name: "cos(0.7853981633974483)",
        xb: 0x3fe921fb54442d18,
        yb: 0x3fe6a09e667f3bcd,
    },
    DetVector {
        name: "sin(1.5707963267331255)",
        xb: 0x3ff921fb543fee68,
        yb: 0x3ff0000000000000,
    },
    DetVector {
        name: "cos(1.5707963267331255)",
        xb: 0x3ff921fb543fee68,
        yb: 0x3dd0fac11a626331,
    },
    DetVector {
        name: "sin(1.5707963267341256)",
        xb: 0x3ff921fb54400000,
        yb: 0x3ff0000000000000,
    },
    DetVector {
        name: "cos(1.5707963267341256)",
        xb: 0x3ff921fb54400000,
        yb: 0x3dd0b4611a626331,
    },
    DetVector {
        name: "sin(1.5707963267351257)",
        xb: 0x3ff921fb54401198,
        yb: 0x3ff0000000000000,
    },
    DetVector {
        name: "cos(1.5707963267351257)",
        xb: 0x3ff921fb54401198,
        yb: 0x3dd06e011a626331,
    },
    DetVector {
        name: "sin(1.413716694115407)",
        xb: 0x3ff69e9565708efc,
        yb: 0x3fef9b24942fe45c,
    },
    DetVector {
        name: "cos(1.413716694115407)",
        xb: 0x3ff69e9565708efc,
        yb: 0x3fc4060b67a85377,
    },
    DetVector {
        name: "sin(3.141592653589793)",
        xb: 0x400921fb54442d18,
        yb: 0x3ca1a62633145c07,
    },
    DetVector {
        name: "cos(3.141592653589793)",
        xb: 0x400921fb54442d18,
        yb: 0xbff0000000000000,
    },
    DetVector {
        name: "sin(2.3561944901011884)",
        xb: 0x4002d97c7f300000,
        yb: 0x3fe6a09e668817b2,
    },
    DetVector {
        name: "cos(2.3561944901011884)",
        xb: 0x4002d97c7f300000,
        yb: 0xbfe6a09e66765fe8,
    },
    DetVector {
        name: "sin(6.283185307179586)",
        xb: 0x401921fb54442d18,
        yb: 0xbcb1a62633145c07,
    },
    DetVector {
        name: "cos(6.283185307179586)",
        xb: 0x401921fb54442d18,
        yb: 0x3ff0000000000000,
    },
    DetVector {
        name: "sin(4.712388980202377)",
        xb: 0x4012d97c7f300000,
        yb: 0xbff0000000000000,
    },
    DetVector {
        name: "cos(4.712388980202377)",
        xb: 0x4012d97c7f300000,
        yb: 0xbde90e91a79394ca,
    },
    DetVector {
        name: "sin(12.566370614359172)",
        xb: 0x402921fb54442d18,
        yb: 0xbcc1a62633145c07,
    },
    DetVector {
        name: "cos(12.566370614359172)",
        xb: 0x402921fb54442d18,
        yb: 0x3ff0000000000000,
    },
    DetVector {
        name: "sin(12.566370613359172)",
        xb: 0x402921fb543b9612,
        yb: 0xbe112e0c8d313198,
    },
    DetVector {
        name: "cos(12.566370613359172)",
        xb: 0x402921fb543b9612,
        yb: 0x3ff0000000000000,
    },
    DetVector {
        name: "tan(0.0)",
        xb: 0x0000000000000000,
        yb: 0x0000000000000000,
    },
    DetVector {
        name: "tan(1e-09)",
        xb: 0x3e112e0be826d695,
        yb: 0x3e112e0be826d695,
    },
    DetVector {
        name: "tan(0.7853981633974483)",
        xb: 0x3fe921fb54442d18,
        yb: 0x3fefffffffffffff,
    },
    DetVector {
        name: "tan(1.0)",
        xb: 0x3ff0000000000000,
        yb: 0x3ff8eb245cbee3a5,
    },
    DetVector {
        name: "tan(1.2)",
        xb: 0x3ff3333333333333,
        yb: 0x400493c43acb164c,
    },
    DetVector {
        name: "tan(1.4)",
        xb: 0x3ff6666666666666,
        yb: 0x401731086dc00a34,
    },
    DetVector {
        name: "tan(1.413716694115407)",
        xb: 0x3ff69e9565708efc,
        yb: 0x4019414813ba6629,
    },
    DetVector {
        name: "tan(1.413716694115407)",
        xb: 0x3ff69e9565708efc,
        yb: 0x4019414813ba6629,
    },
];

pub(crate) const POW_VECTORS: &[PowVector] = &[
    PowVector {
        cb: 0x0000000000000000,
        rb: 0x3ff1a305532617c2,
        yb: 0x0000000000000000,
    }, // pow(0.0, 1.1023)
    PowVector {
        cb: 0x3ff0000000000000,
        rb: 0x40160ccccccccccd,
        yb: 0x3ff0000000000000,
    }, // pow(1.0, 5.5125)
    PowVector {
        cb: 0x3feffb15b573eab3,
        rb: 0x3ff0000000000000,
        yb: 0x3feffb15b573eab3,
    }, // pow(0.9994, 1.0)
    PowVector {
        cb: 0x3feffb15b573eab3,
        rb: 0x40116a3b35fc845b,
        yb: 0x3fefea9f3c0f5062,
    }, // pow(0.9994, 4.35374149659864)
    PowVector {
        cb: 0x3fefd70a3d70a3d7,
        rb: 0x3ff16a161e4f7660,
        yb: 0x3fefd36dd32a4d29,
    }, // pow(0.995, 1.0884)
    PowVector {
        cb: 0x3fefae147ae147ae,
        rb: 0x40060ccccccccccd,
        yb: 0x3fef202f8c50da39,
    }, // pow(0.99, 2.75625)
    PowVector {
        cb: 0x3fa47ae147ae147b,
        rb: 0x3f97384c77a7d74a,
        yb: 0x3fedbf5d3929e980,
    }, // pow(0.04, 0.0226757)
    PowVector {
        cb: 0x3fa47ae147ae147b,
        rb: 0x3f87384ef2a605ce,
        yb: 0x3feeda6bcc96b012,
    }, // pow(0.04, 0.011337868480725623)
    PowVector {
        cb: 0x3fe0000000000000,
        rb: 0x40160ccccccccccd,
        yb: 0x3f966ea576492e52,
    }, // pow(0.5, 5.5125)
    PowVector {
        cb: 0x3fefff2e48e8a71e,
        rb: 0x40116a305532617c,
        yb: 0x3feffc6f1db3c5e7,
    }, // pow(0.9999, 4.3537)
];
