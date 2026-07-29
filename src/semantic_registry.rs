use crate::ScalarType;
use std::io::Write;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PrimitiveId(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SignatureId(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ImplementationId(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ApplicationPlanId(u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Conversion {
    Identity,
    PromoteIntToDouble,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperandConsumption {
    Elementwise,
    WholeVector,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OperandDescriptor {
    pub element_type: ScalarType,
    pub consumption: OperandConsumption,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum ResultCardinality {
    Elementwise,
    Scalar,
    DynamicVector,
    PreserveOperand(u16),
    OperandPlusOne(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum WorkAdmission {
    Constant(u32),
    ResultCardinality,
    OperandCardinality(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResourceAdmissionPlan {
    pub work: WorkAdmission,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationPlan {
    pub id: ApplicationPlanId,
    pub result_cardinality: ResultCardinality,
    pub resources: ResourceAdmissionPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StructuralBehavior {
    Elementwise,
    Iota,
    VectorLength,
    VectorSort,
    VectorSum,
    VectorAllOf,
    VectorAnyOf,
    VectorNoneOf,
    Foldl,
    Scanl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarKernel {
    IncInt,
    IncDouble,
    DecInt,
    DecDouble,
    NegInt,
    NegDouble,
    AbsInt,
    AbsDouble,
    AddInt,
    AddDouble,
    SubInt,
    SubDouble,
    MulInt,
    MulDouble,
    DivInt,
    DivDouble,
    LengthBoolVector,
    LengthIntVector,
    LengthDoubleVector,
    SortBoolVector,
    SortIntVector,
    SortDoubleVector,
    SumIntVector,
    SumDoubleVector,
    AllOfBoolVector,
    AnyOfBoolVector,
    NoneOfBoolVector,
    FoldlBool,
    FoldlInt,
    FoldlDouble,
    ScanlBool,
    ScanlInt,
    ScanlDouble,
    EqualsBool,
    EqualsInt,
    EqualsDouble,
    NotEqualsBool,
    NotEqualsInt,
    NotEqualsDouble,
    NotBool,
    AndBool,
    OrBool,
    OddInt,
    EvenInt,
    IsPositiveInt,
    IsPositiveDouble,
    IsNegativeInt,
    IsNegativeDouble,
    LessThanInt,
    LessThanDouble,
    GreaterThanInt,
    GreaterThanDouble,
    IotaInt,
    SqrtDouble,
    ExpDouble,
    LogDouble,
    Log10Double,
    SinDouble,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SemanticDescriptor {
    pub primitive_id: PrimitiveId,
    pub primitive_name: &'static str,
    pub signature_id: SignatureId,
    pub implementation_id: ImplementationId,
    pub parameters: &'static [OperandDescriptor],
    pub result: ScalarType,
    pub behavior: StructuralBehavior,
    pub application_plan: ApplicationPlan,
    pub kernel: ScalarKernel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum RegistryLookupError {
    PrimitiveName,
    PrimitiveId,
    SignatureId,
    ImplementationId,
    ApplicationPlanId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistryValidationError {
    DuplicatePrimitiveName,
    DuplicateSignatureId,
    DuplicateImplementationId,
    MissingPrimitiveId,
    MissingSignatureId,
    MissingImplementationId,
    UnknownPrimitiveId,
    UnknownSignatureId,
    UnknownImplementationId,
    UnknownApplicationPlanId,
    DuplicateApplicationPlanId,
    MissingApplicationPlanId,
    InvalidApplicationPlan,
    InconsistentPrimitiveIdentity,
    InconsistentSignatureIdentity,
    InconsistentImplementationIdentity,
    InconsistentApplicationPlanIdentity,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InternalRegistryDiagnosticError {
    InvalidRegistry,
    AllocationUnavailable,
    WriteFailed,
    FlushFailed,
}

impl InternalRegistryDiagnosticError {
    pub const fn diagnostic(self) -> &'static str {
        match self {
            Self::InvalidRegistry => "production registry is invalid",
            Self::AllocationUnavailable => "unable to allocate registry diagnostics",
            Self::WriteFailed => "unable to write registry diagnostics",
            Self::FlushFailed => "unable to flush registry diagnostics",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct InternalRegistryDiagnosticFailureInjection {
    refuse_reservation: bool,
}

impl InternalRegistryDiagnosticFailureInjection {
    const fn none() -> Self {
        Self {
            refuse_reservation: false,
        }
    }

    #[cfg(test)]
    const fn refuse_reservation() -> Self {
        Self {
            refuse_reservation: true,
        }
    }
}

const PRIMITIVE_COUNT: u16 = 33;
const SIGNATURE_COUNT: u16 = 58;
const IMPLEMENTATION_COUNT: u16 = 58;
const APPLICATION_PLAN_COUNT: u16 = 10;

const fn elementwise(element_type: ScalarType) -> OperandDescriptor {
    OperandDescriptor {
        element_type,
        consumption: OperandConsumption::Elementwise,
    }
}

const fn whole_vector(element_type: ScalarType) -> OperandDescriptor {
    OperandDescriptor {
        element_type,
        consumption: OperandConsumption::WholeVector,
    }
}

const INT1: &[OperandDescriptor] = &[elementwise(ScalarType::Int)];
const DOUBLE1: &[OperandDescriptor] = &[elementwise(ScalarType::Double)];
const BOOL1: &[OperandDescriptor] = &[elementwise(ScalarType::Bool)];
const INT2: &[OperandDescriptor] = &[elementwise(ScalarType::Int), elementwise(ScalarType::Int)];
const DOUBLE2: &[OperandDescriptor] = &[
    elementwise(ScalarType::Double),
    elementwise(ScalarType::Double),
];
const BOOL2: &[OperandDescriptor] = &[elementwise(ScalarType::Bool), elementwise(ScalarType::Bool)];
const WHOLE_BOOL1: &[OperandDescriptor] = &[whole_vector(ScalarType::Bool)];
const WHOLE_INT1: &[OperandDescriptor] = &[whole_vector(ScalarType::Int)];
const WHOLE_DOUBLE1: &[OperandDescriptor] = &[whole_vector(ScalarType::Double)];
const FOLDL_BOOL: &[OperandDescriptor] = &[
    elementwise(ScalarType::Bool),
    whole_vector(ScalarType::Bool),
];
const FOLDL_INT: &[OperandDescriptor] =
    &[elementwise(ScalarType::Int), whole_vector(ScalarType::Int)];
const FOLDL_DOUBLE: &[OperandDescriptor] = &[
    elementwise(ScalarType::Double),
    whole_vector(ScalarType::Double),
];

const ELEMENTWISE_PLAN: ApplicationPlan = ApplicationPlan {
    id: ApplicationPlanId(1),
    result_cardinality: ResultCardinality::Elementwise,
    resources: ResourceAdmissionPlan {
        work: WorkAdmission::ResultCardinality,
    },
};

const IOTA_PLAN: ApplicationPlan = ApplicationPlan {
    id: ApplicationPlanId(2),
    result_cardinality: ResultCardinality::DynamicVector,
    resources: ResourceAdmissionPlan {
        work: WorkAdmission::ResultCardinality,
    },
};

const LENGTH_PLAN: ApplicationPlan = ApplicationPlan {
    id: ApplicationPlanId(3),
    result_cardinality: ResultCardinality::Scalar,
    resources: ResourceAdmissionPlan {
        work: WorkAdmission::Constant(1),
    },
};

const SORT_PLAN: ApplicationPlan = ApplicationPlan {
    id: ApplicationPlanId(4),
    result_cardinality: ResultCardinality::PreserveOperand(1),
    resources: ResourceAdmissionPlan {
        work: WorkAdmission::OperandCardinality(1),
    },
};

const SUM_PLAN: ApplicationPlan = ApplicationPlan {
    id: ApplicationPlanId(5),
    result_cardinality: ResultCardinality::Scalar,
    resources: ResourceAdmissionPlan {
        work: WorkAdmission::OperandCardinality(1),
    },
};

const ALL_OF_PLAN: ApplicationPlan = ApplicationPlan {
    id: ApplicationPlanId(6),
    result_cardinality: ResultCardinality::Scalar,
    resources: ResourceAdmissionPlan {
        work: WorkAdmission::OperandCardinality(1),
    },
};

const ANY_OF_PLAN: ApplicationPlan = ApplicationPlan {
    id: ApplicationPlanId(7),
    result_cardinality: ResultCardinality::Scalar,
    resources: ResourceAdmissionPlan {
        work: WorkAdmission::OperandCardinality(1),
    },
};

const NONE_OF_PLAN: ApplicationPlan = ApplicationPlan {
    id: ApplicationPlanId(8),
    result_cardinality: ResultCardinality::Scalar,
    resources: ResourceAdmissionPlan {
        work: WorkAdmission::OperandCardinality(1),
    },
};

const FOLDL_PLAN: ApplicationPlan = ApplicationPlan {
    id: ApplicationPlanId(9),
    result_cardinality: ResultCardinality::Scalar,
    resources: ResourceAdmissionPlan {
        work: WorkAdmission::OperandCardinality(2),
    },
};

const SCANL_PLAN: ApplicationPlan = ApplicationPlan {
    id: ApplicationPlanId(10),
    result_cardinality: ResultCardinality::OperandPlusOne(2),
    resources: ResourceAdmissionPlan {
        work: WorkAdmission::OperandCardinality(2),
    },
};

const APPLICATION_PLANS: &[ApplicationPlan] = &[
    ELEMENTWISE_PLAN,
    IOTA_PLAN,
    LENGTH_PLAN,
    SORT_PLAN,
    SUM_PLAN,
    ALL_OF_PLAN,
    ANY_OF_PLAN,
    NONE_OF_PLAN,
    FOLDL_PLAN,
    SCANL_PLAN,
];

pub(crate) const BACKEND_NATIVE_MATH_FIRST_PRIMITIVE_ID: u16 = 29;
pub(crate) const BACKEND_NATIVE_MATH_LAST_PRIMITIVE_ID: u16 = 38;

pub(crate) const fn is_backend_native_math_primitive(primitive_id: u16) -> bool {
    primitive_id >= BACKEND_NATIVE_MATH_FIRST_PRIMITIVE_ID
        && primitive_id <= BACKEND_NATIVE_MATH_LAST_PRIMITIVE_ID
}

macro_rules! descriptor {
    ($primitive:literal, $name:literal, $signature:literal, $implementation:literal,
     $parameters:ident, $result:ident, $behavior:ident, $plan:ident, $kernel:ident) => {
        SemanticDescriptor {
            primitive_id: PrimitiveId($primitive),
            primitive_name: $name,
            signature_id: SignatureId($signature),
            implementation_id: ImplementationId($implementation),
            parameters: $parameters,
            result: ScalarType::$result,
            behavior: StructuralBehavior::$behavior,
            application_plan: $plan,
            kernel: ScalarKernel::$kernel,
        }
    };
}

// This is the single production owner of primitive names and all stable semantic IDs.
pub(crate) const SEMANTIC_REGISTRY: &[SemanticDescriptor] = &[
    descriptor!(
        1,
        "inc",
        1,
        1,
        INT1,
        Int,
        Elementwise,
        ELEMENTWISE_PLAN,
        IncInt
    ),
    descriptor!(
        1,
        "inc",
        2,
        2,
        DOUBLE1,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        IncDouble
    ),
    descriptor!(
        2,
        "dec",
        3,
        3,
        INT1,
        Int,
        Elementwise,
        ELEMENTWISE_PLAN,
        DecInt
    ),
    descriptor!(
        2,
        "dec",
        4,
        4,
        DOUBLE1,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        DecDouble
    ),
    descriptor!(
        3,
        "neg",
        5,
        5,
        INT1,
        Int,
        Elementwise,
        ELEMENTWISE_PLAN,
        NegInt
    ),
    descriptor!(
        3,
        "neg",
        6,
        6,
        DOUBLE1,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        NegDouble
    ),
    descriptor!(
        4,
        "abs",
        7,
        7,
        INT1,
        Int,
        Elementwise,
        ELEMENTWISE_PLAN,
        AbsInt
    ),
    descriptor!(
        4,
        "abs",
        8,
        8,
        DOUBLE1,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        AbsDouble
    ),
    descriptor!(
        5,
        "add",
        9,
        9,
        INT2,
        Int,
        Elementwise,
        ELEMENTWISE_PLAN,
        AddInt
    ),
    descriptor!(
        5,
        "add",
        10,
        10,
        DOUBLE2,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        AddDouble
    ),
    descriptor!(
        6,
        "sub",
        11,
        11,
        INT2,
        Int,
        Elementwise,
        ELEMENTWISE_PLAN,
        SubInt
    ),
    descriptor!(
        6,
        "sub",
        12,
        12,
        DOUBLE2,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        SubDouble
    ),
    descriptor!(
        7,
        "mul",
        13,
        13,
        INT2,
        Int,
        Elementwise,
        ELEMENTWISE_PLAN,
        MulInt
    ),
    descriptor!(
        7,
        "mul",
        14,
        14,
        DOUBLE2,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        MulDouble
    ),
    descriptor!(
        8,
        "equals",
        15,
        15,
        BOOL2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        EqualsBool
    ),
    descriptor!(
        8,
        "equals",
        16,
        16,
        INT2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        EqualsInt
    ),
    descriptor!(
        8,
        "equals",
        17,
        17,
        DOUBLE2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        EqualsDouble
    ),
    descriptor!(
        9,
        "not_equals",
        18,
        18,
        BOOL2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        NotEqualsBool
    ),
    descriptor!(
        9,
        "not_equals",
        19,
        19,
        INT2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        NotEqualsInt
    ),
    descriptor!(
        9,
        "not_equals",
        20,
        20,
        DOUBLE2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        NotEqualsDouble
    ),
    descriptor!(
        10,
        "not",
        21,
        21,
        BOOL1,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        NotBool
    ),
    descriptor!(
        11,
        "and",
        22,
        22,
        BOOL2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        AndBool
    ),
    descriptor!(
        12,
        "or",
        23,
        23,
        BOOL2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        OrBool
    ),
    descriptor!(
        13,
        "odd",
        24,
        24,
        INT1,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        OddInt
    ),
    descriptor!(
        14,
        "even",
        25,
        25,
        INT1,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        EvenInt
    ),
    descriptor!(
        15,
        "is_positive",
        26,
        26,
        INT1,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        IsPositiveInt
    ),
    descriptor!(
        15,
        "is_positive",
        27,
        27,
        DOUBLE1,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        IsPositiveDouble
    ),
    descriptor!(
        16,
        "is_negative",
        28,
        28,
        INT1,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        IsNegativeInt
    ),
    descriptor!(
        16,
        "is_negative",
        29,
        29,
        DOUBLE1,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        IsNegativeDouble
    ),
    descriptor!(
        17,
        "less_than",
        30,
        30,
        INT2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        LessThanInt
    ),
    descriptor!(
        17,
        "less_than",
        31,
        31,
        DOUBLE2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        LessThanDouble
    ),
    descriptor!(
        18,
        "greater_than",
        32,
        32,
        INT2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        GreaterThanInt
    ),
    descriptor!(
        18,
        "greater_than",
        33,
        33,
        DOUBLE2,
        Bool,
        Elementwise,
        ELEMENTWISE_PLAN,
        GreaterThanDouble
    ),
    descriptor!(19, "iota", 34, 34, INT1, Int, Iota, IOTA_PLAN, IotaInt),
    descriptor!(
        20,
        "div",
        35,
        35,
        INT2,
        Int,
        Elementwise,
        ELEMENTWISE_PLAN,
        DivInt
    ),
    descriptor!(
        20,
        "div",
        36,
        36,
        DOUBLE2,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        DivDouble
    ),
    descriptor!(
        21,
        "length",
        37,
        37,
        WHOLE_BOOL1,
        Int,
        VectorLength,
        LENGTH_PLAN,
        LengthBoolVector
    ),
    descriptor!(
        21,
        "length",
        38,
        38,
        WHOLE_INT1,
        Int,
        VectorLength,
        LENGTH_PLAN,
        LengthIntVector
    ),
    descriptor!(
        21,
        "length",
        39,
        39,
        WHOLE_DOUBLE1,
        Int,
        VectorLength,
        LENGTH_PLAN,
        LengthDoubleVector
    ),
    descriptor!(
        22,
        "sort",
        40,
        40,
        WHOLE_BOOL1,
        Bool,
        VectorSort,
        SORT_PLAN,
        SortBoolVector
    ),
    descriptor!(
        22,
        "sort",
        41,
        41,
        WHOLE_INT1,
        Int,
        VectorSort,
        SORT_PLAN,
        SortIntVector
    ),
    descriptor!(
        22,
        "sort",
        42,
        42,
        WHOLE_DOUBLE1,
        Double,
        VectorSort,
        SORT_PLAN,
        SortDoubleVector
    ),
    descriptor!(
        23,
        "sum",
        43,
        43,
        WHOLE_INT1,
        Int,
        VectorSum,
        SUM_PLAN,
        SumIntVector
    ),
    descriptor!(
        23,
        "sum",
        44,
        44,
        WHOLE_DOUBLE1,
        Double,
        VectorSum,
        SUM_PLAN,
        SumDoubleVector
    ),
    descriptor!(
        24,
        "all_of",
        45,
        45,
        WHOLE_BOOL1,
        Bool,
        VectorAllOf,
        ALL_OF_PLAN,
        AllOfBoolVector
    ),
    descriptor!(
        25,
        "any_of",
        46,
        46,
        WHOLE_BOOL1,
        Bool,
        VectorAnyOf,
        ANY_OF_PLAN,
        AnyOfBoolVector
    ),
    descriptor!(
        26,
        "none_of",
        47,
        47,
        WHOLE_BOOL1,
        Bool,
        VectorNoneOf,
        NONE_OF_PLAN,
        NoneOfBoolVector
    ),
    descriptor!(
        27, "foldl", 48, 48, FOLDL_BOOL, Bool, Foldl, FOLDL_PLAN, FoldlBool
    ),
    descriptor!(
        27, "foldl", 49, 49, FOLDL_INT, Int, Foldl, FOLDL_PLAN, FoldlInt
    ),
    descriptor!(
        27,
        "foldl",
        50,
        50,
        FOLDL_DOUBLE,
        Double,
        Foldl,
        FOLDL_PLAN,
        FoldlDouble
    ),
    descriptor!(
        28, "scanl", 51, 51, FOLDL_BOOL, Bool, Scanl, SCANL_PLAN, ScanlBool
    ),
    descriptor!(
        28, "scanl", 52, 52, FOLDL_INT, Int, Scanl, SCANL_PLAN, ScanlInt
    ),
    descriptor!(
        28,
        "scanl",
        53,
        53,
        FOLDL_DOUBLE,
        Double,
        Scanl,
        SCANL_PLAN,
        ScanlDouble
    ),
    descriptor!(
        29,
        "sqrt",
        54,
        54,
        DOUBLE1,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        SqrtDouble
    ),
    descriptor!(
        30,
        "exp",
        55,
        55,
        DOUBLE1,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        ExpDouble
    ),
    descriptor!(
        31,
        "log",
        56,
        56,
        DOUBLE1,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        LogDouble
    ),
    descriptor!(
        32,
        "log10",
        57,
        57,
        DOUBLE1,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        Log10Double
    ),
    descriptor!(
        33,
        "sin",
        58,
        58,
        DOUBLE1,
        Double,
        Elementwise,
        ELEMENTWISE_PLAN,
        SinDouble
    ),
];

impl PrimitiveId {
    #[allow(dead_code)]
    pub(crate) const fn numeric(self) -> u16 {
        self.0
    }
}

impl SignatureId {
    #[allow(dead_code)]
    pub(crate) const fn numeric(self) -> u16 {
        self.0
    }
}

impl ImplementationId {
    #[allow(dead_code)]
    pub(crate) const fn numeric(self) -> u16 {
        self.0
    }
}

impl ApplicationPlanId {
    pub(crate) const fn numeric(self) -> u16 {
        self.0
    }
}

pub(crate) fn primitive_from_name(name: &str) -> Result<PrimitiveId, RegistryLookupError> {
    SEMANTIC_REGISTRY
        .iter()
        .find(|descriptor| descriptor.primitive_name == name)
        .map(|descriptor| descriptor.primitive_id)
        .ok_or(RegistryLookupError::PrimitiveName)
}

#[allow(dead_code)]
pub(crate) fn primitive_from_numeric(numeric: u16) -> Result<PrimitiveId, RegistryLookupError> {
    SEMANTIC_REGISTRY
        .iter()
        .find(|descriptor| descriptor.primitive_id.numeric() == numeric)
        .map(|descriptor| descriptor.primitive_id)
        .ok_or(RegistryLookupError::PrimitiveId)
}

#[allow(dead_code)]
pub(crate) fn signature_from_numeric(
    numeric: u16,
) -> Result<&'static SemanticDescriptor, RegistryLookupError> {
    SEMANTIC_REGISTRY
        .iter()
        .find(|descriptor| descriptor.signature_id.numeric() == numeric)
        .ok_or(RegistryLookupError::SignatureId)
}

#[allow(dead_code)]
pub(crate) fn implementation_from_numeric(
    numeric: u16,
) -> Result<&'static SemanticDescriptor, RegistryLookupError> {
    SEMANTIC_REGISTRY
        .iter()
        .find(|descriptor| descriptor.implementation_id.numeric() == numeric)
        .ok_or(RegistryLookupError::ImplementationId)
}

pub(crate) fn application_plan_from_numeric(
    numeric: u16,
) -> Result<ApplicationPlan, RegistryLookupError> {
    APPLICATION_PLANS
        .iter()
        .find(|plan| plan.id.numeric() == numeric)
        .copied()
        .ok_or(RegistryLookupError::ApplicationPlanId)
}

pub(crate) fn descriptors(
    primitive: PrimitiveId,
) -> impl Iterator<Item = &'static SemanticDescriptor> {
    SEMANTIC_REGISTRY
        .iter()
        .filter(move |descriptor| descriptor.primitive_id == primitive)
}

pub(crate) fn conversion(actual: ScalarType, accepted: ScalarType) -> Option<Conversion> {
    if actual == accepted {
        Some(Conversion::Identity)
    } else if actual == ScalarType::Int && accepted == ScalarType::Double {
        Some(Conversion::PromoteIntToDouble)
    } else {
        None
    }
}

/// Writes a human-readable view of the production semantic registry.
///
/// This is internal diagnostic output: its spelling and layout are not a stable
/// machine-readable interface and may change without compatibility guarantees.
#[doc(hidden)]
pub fn write_internal_registry_diagnostics(
    output: &mut impl Write,
) -> Result<(), InternalRegistryDiagnosticError> {
    write_registry_diagnostics(
        SEMANTIC_REGISTRY,
        output,
        InternalRegistryDiagnosticFailureInjection::none(),
    )
}

fn write_registry_diagnostics(
    registry: &[SemanticDescriptor],
    output: &mut impl Write,
    injection: InternalRegistryDiagnosticFailureInjection,
) -> Result<(), InternalRegistryDiagnosticError> {
    validate_registry(registry).map_err(|_| InternalRegistryDiagnosticError::InvalidRegistry)?;

    let mut ordered: Vec<&SemanticDescriptor> = Vec::new();
    if injection.refuse_reservation || ordered.try_reserve_exact(registry.len()).is_err() {
        return Err(InternalRegistryDiagnosticError::AllocationUnavailable);
    }
    ordered.extend(registry);
    ordered.sort_unstable_by_key(|descriptor| {
        (
            descriptor.primitive_id.numeric(),
            descriptor.signature_id.numeric(),
            descriptor.implementation_id.numeric(),
        )
    });

    writeln!(
        output,
        "Faraweave semantic registry (internal human-readable diagnostics; format is unstable)"
    )
    .map_err(|_| InternalRegistryDiagnosticError::WriteFailed)?;
    let mut previous_primitive = None;
    for descriptor in ordered {
        if previous_primitive != Some(descriptor.primitive_id) {
            writeln!(
                output,
                "primitive id={} name={} behavior={}",
                descriptor.primitive_id.numeric(),
                descriptor.primitive_name,
                structural_behavior_name(descriptor.behavior),
            )
            .map_err(|_| InternalRegistryDiagnosticError::WriteFailed)?;
            previous_primitive = Some(descriptor.primitive_id);
        }
        write!(
            output,
            "  signature id={} implementation={} parameters=[",
            descriptor.signature_id.numeric(),
            descriptor.implementation_id.numeric(),
        )
        .map_err(|_| InternalRegistryDiagnosticError::WriteFailed)?;
        for (index, parameter) in descriptor.parameters.iter().enumerate() {
            if index != 0 {
                write!(output, "; ").map_err(|_| InternalRegistryDiagnosticError::WriteFailed)?;
            }
            write!(
                output,
                "{}:accepted={},lift={},actual={{",
                index + 1,
                parameter.element_type.name(),
                operand_consumption_name(parameter.consumption),
            )
            .map_err(|_| InternalRegistryDiagnosticError::WriteFailed)?;
            write_conversions(output, *parameter)?;
            write!(output, "}}").map_err(|_| InternalRegistryDiagnosticError::WriteFailed)?;
        }
        write!(
            output,
            "] result={} application_plan={} result_cardinality=",
            descriptor.result.name(),
            descriptor.application_plan.id.numeric(),
        )
        .map_err(|_| InternalRegistryDiagnosticError::WriteFailed)?;
        write_result_cardinality(output, descriptor.application_plan.result_cardinality)?;
        write!(output, " work=").map_err(|_| InternalRegistryDiagnosticError::WriteFailed)?;
        write_work_admission(output, descriptor.application_plan.resources.work)?;
        writeln!(output, " kernel={}", scalar_kernel_name(descriptor.kernel),)
            .map_err(|_| InternalRegistryDiagnosticError::WriteFailed)?;
    }
    output
        .flush()
        .map_err(|_| InternalRegistryDiagnosticError::FlushFailed)
}

fn write_conversions(
    output: &mut impl Write,
    parameter: OperandDescriptor,
) -> Result<(), InternalRegistryDiagnosticError> {
    let mut first = true;
    for actual in [ScalarType::Bool, ScalarType::Int, ScalarType::Double] {
        let Some(selected) = conversion(actual, parameter.element_type) else {
            continue;
        };
        if parameter.consumption == OperandConsumption::WholeVector
            && selected != Conversion::Identity
        {
            continue;
        }
        if !first {
            write!(output, ",").map_err(|_| InternalRegistryDiagnosticError::WriteFailed)?;
        }
        write!(output, "{}:{}", actual.name(), conversion_name(selected))
            .map_err(|_| InternalRegistryDiagnosticError::WriteFailed)?;
        first = false;
    }
    Ok(())
}

const fn conversion_name(value: Conversion) -> &'static str {
    match value {
        Conversion::Identity => "identity",
        Conversion::PromoteIntToDouble => "promote_int_to_double",
    }
}

const fn operand_consumption_name(value: OperandConsumption) -> &'static str {
    match value {
        OperandConsumption::Elementwise => "elementwise",
        OperandConsumption::WholeVector => "whole_vector",
    }
}

fn write_result_cardinality(
    output: &mut impl Write,
    value: ResultCardinality,
) -> Result<(), InternalRegistryDiagnosticError> {
    match value {
        ResultCardinality::Elementwise => write!(output, "elementwise"),
        ResultCardinality::Scalar => write!(output, "scalar"),
        ResultCardinality::DynamicVector => write!(output, "dynamic_vector"),
        ResultCardinality::PreserveOperand(position) => {
            write!(output, "preserve_operand({position})")
        }
        ResultCardinality::OperandPlusOne(position) => {
            write!(output, "operand_plus_one({position})")
        }
    }
    .map_err(|_| InternalRegistryDiagnosticError::WriteFailed)
}

fn write_work_admission(
    output: &mut impl Write,
    value: WorkAdmission,
) -> Result<(), InternalRegistryDiagnosticError> {
    match value {
        WorkAdmission::Constant(work) => write!(output, "constant({work})"),
        WorkAdmission::ResultCardinality => write!(output, "result_cardinality"),
        WorkAdmission::OperandCardinality(position) => {
            write!(output, "operand_cardinality({position})")
        }
    }
    .map_err(|_| InternalRegistryDiagnosticError::WriteFailed)
}

const fn structural_behavior_name(value: StructuralBehavior) -> &'static str {
    match value {
        StructuralBehavior::Elementwise => "elementwise",
        StructuralBehavior::Iota => "iota",
        StructuralBehavior::VectorLength => "vector_length",
        StructuralBehavior::VectorSort => "vector_sort",
        StructuralBehavior::VectorSum => "vector_sum",
        StructuralBehavior::VectorAllOf => "vector_all_of",
        StructuralBehavior::VectorAnyOf => "vector_any_of",
        StructuralBehavior::VectorNoneOf => "vector_none_of",
        StructuralBehavior::Foldl => "foldl",
        StructuralBehavior::Scanl => "scanl",
    }
}

const fn scalar_kernel_name(value: ScalarKernel) -> &'static str {
    match value {
        ScalarKernel::IncInt => "inc_int",
        ScalarKernel::IncDouble => "inc_double",
        ScalarKernel::DecInt => "dec_int",
        ScalarKernel::DecDouble => "dec_double",
        ScalarKernel::NegInt => "neg_int",
        ScalarKernel::NegDouble => "neg_double",
        ScalarKernel::AbsInt => "abs_int",
        ScalarKernel::AbsDouble => "abs_double",
        ScalarKernel::AddInt => "add_int",
        ScalarKernel::AddDouble => "add_double",
        ScalarKernel::SubInt => "sub_int",
        ScalarKernel::SubDouble => "sub_double",
        ScalarKernel::MulInt => "mul_int",
        ScalarKernel::MulDouble => "mul_double",
        ScalarKernel::DivInt => "div_int",
        ScalarKernel::DivDouble => "div_double",
        ScalarKernel::EqualsBool => "equals_bool",
        ScalarKernel::EqualsInt => "equals_int",
        ScalarKernel::EqualsDouble => "equals_double",
        ScalarKernel::NotEqualsBool => "not_equals_bool",
        ScalarKernel::NotEqualsInt => "not_equals_int",
        ScalarKernel::NotEqualsDouble => "not_equals_double",
        ScalarKernel::NotBool => "not_bool",
        ScalarKernel::AndBool => "and_bool",
        ScalarKernel::OrBool => "or_bool",
        ScalarKernel::OddInt => "odd_int",
        ScalarKernel::EvenInt => "even_int",
        ScalarKernel::IsPositiveInt => "is_positive_int",
        ScalarKernel::IsPositiveDouble => "is_positive_double",
        ScalarKernel::IsNegativeInt => "is_negative_int",
        ScalarKernel::IsNegativeDouble => "is_negative_double",
        ScalarKernel::LessThanInt => "less_than_int",
        ScalarKernel::LessThanDouble => "less_than_double",
        ScalarKernel::GreaterThanInt => "greater_than_int",
        ScalarKernel::GreaterThanDouble => "greater_than_double",
        ScalarKernel::IotaInt => "iota_int",
        ScalarKernel::LengthBoolVector => "length_bool_vector",
        ScalarKernel::LengthIntVector => "length_int_vector",
        ScalarKernel::LengthDoubleVector => "length_double_vector",
        ScalarKernel::SortBoolVector => "sort_bool_vector",
        ScalarKernel::SortIntVector => "sort_int_vector",
        ScalarKernel::SortDoubleVector => "sort_double_vector",
        ScalarKernel::SumIntVector => "sum_int_vector",
        ScalarKernel::SumDoubleVector => "sum_double_vector",
        ScalarKernel::AllOfBoolVector => "all_of_bool_vector",
        ScalarKernel::AnyOfBoolVector => "any_of_bool_vector",
        ScalarKernel::NoneOfBoolVector => "none_of_bool_vector",
        ScalarKernel::FoldlBool => "foldl_bool",
        ScalarKernel::FoldlInt => "foldl_int",
        ScalarKernel::FoldlDouble => "foldl_double",
        ScalarKernel::ScanlBool => "scanl_bool",
        ScalarKernel::ScanlInt => "scanl_int",
        ScalarKernel::ScanlDouble => "scanl_double",
        ScalarKernel::SqrtDouble => "sqrt_double",
        ScalarKernel::ExpDouble => "exp_double",
        ScalarKernel::LogDouble => "log_double",
        ScalarKernel::Log10Double => "log10_double",
        ScalarKernel::SinDouble => "sin_double",
    }
}

#[allow(dead_code)]
fn validate_registry(registry: &[SemanticDescriptor]) -> Result<(), RegistryValidationError> {
    validate_registry_with_application_plans(registry, APPLICATION_PLANS)
}

fn validate_registry_with_application_plans(
    registry: &[SemanticDescriptor],
    application_plans: &[ApplicationPlan],
) -> Result<(), RegistryValidationError> {
    validate_application_plan_catalog(application_plans)?;

    for descriptor in registry {
        if descriptor.primitive_id.numeric() == 0
            || descriptor.primitive_id.numeric() > PRIMITIVE_COUNT
        {
            return Err(RegistryValidationError::UnknownPrimitiveId);
        }
        if descriptor.signature_id.numeric() == 0
            || descriptor.signature_id.numeric() > SIGNATURE_COUNT
        {
            return Err(RegistryValidationError::UnknownSignatureId);
        }
        if descriptor.implementation_id.numeric() == 0
            || descriptor.implementation_id.numeric() > IMPLEMENTATION_COUNT
        {
            return Err(RegistryValidationError::UnknownImplementationId);
        }
        if descriptor.application_plan.id.numeric() == 0
            || descriptor.application_plan.id.numeric() > APPLICATION_PLAN_COUNT
        {
            return Err(RegistryValidationError::UnknownApplicationPlanId);
        }
        if !valid_application_plan(descriptor) {
            return Err(RegistryValidationError::InvalidApplicationPlan);
        }
        if application_plans
            .iter()
            .find(|plan| plan.id == descriptor.application_plan.id)
            .is_none_or(|plan| *plan != descriptor.application_plan)
        {
            return Err(RegistryValidationError::InconsistentApplicationPlanIdentity);
        }
    }

    for (index, descriptor) in registry.iter().enumerate() {
        for other in registry.iter().skip(index + 1) {
            if descriptor.signature_id == other.signature_id {
                return Err(RegistryValidationError::DuplicateSignatureId);
            }
            if descriptor.implementation_id == other.implementation_id {
                return Err(RegistryValidationError::DuplicateImplementationId);
            }
            if descriptor.primitive_name == other.primitive_name
                && descriptor.primitive_id != other.primitive_id
            {
                return Err(RegistryValidationError::DuplicatePrimitiveName);
            }
            if descriptor.primitive_id == other.primitive_id
                && (descriptor.primitive_name != other.primitive_name
                    || descriptor.behavior != other.behavior)
            {
                return Err(RegistryValidationError::InconsistentPrimitiveIdentity);
            }
        }
    }

    for expected in 1..=PRIMITIVE_COUNT {
        if !registry
            .iter()
            .any(|descriptor| descriptor.primitive_id.numeric() == expected)
        {
            return Err(RegistryValidationError::MissingPrimitiveId);
        }
    }
    for expected in 1..=SIGNATURE_COUNT {
        if !registry
            .iter()
            .any(|descriptor| descriptor.signature_id.numeric() == expected)
        {
            return Err(RegistryValidationError::MissingSignatureId);
        }
    }
    for expected in 1..=IMPLEMENTATION_COUNT {
        if !registry
            .iter()
            .any(|descriptor| descriptor.implementation_id.numeric() == expected)
        {
            return Err(RegistryValidationError::MissingImplementationId);
        }
    }

    for descriptor in registry {
        let canonical_primitive = SEMANTIC_REGISTRY
            .iter()
            .find(|canonical| canonical.primitive_id == descriptor.primitive_id);
        if canonical_primitive.is_none_or(|canonical| {
            descriptor.primitive_name != canonical.primitive_name
                || descriptor.behavior != canonical.behavior
        }) {
            return Err(RegistryValidationError::InconsistentPrimitiveIdentity);
        }

        let canonical_signature = SEMANTIC_REGISTRY
            .iter()
            .find(|canonical| canonical.signature_id == descriptor.signature_id);
        if canonical_signature.is_none_or(|canonical| {
            descriptor.primitive_id != canonical.primitive_id
                || descriptor.parameters != canonical.parameters
                || descriptor.result != canonical.result
                || descriptor.behavior != canonical.behavior
                || descriptor.application_plan != canonical.application_plan
        }) {
            return Err(RegistryValidationError::InconsistentSignatureIdentity);
        }

        let canonical_implementation = SEMANTIC_REGISTRY
            .iter()
            .find(|canonical| canonical.implementation_id == descriptor.implementation_id);
        if canonical_implementation.is_none_or(|canonical| {
            descriptor.primitive_id != canonical.primitive_id
                || descriptor.parameters != canonical.parameters
                || descriptor.result != canonical.result
                || descriptor.behavior != canonical.behavior
                || descriptor.application_plan != canonical.application_plan
                || descriptor.kernel != canonical.kernel
        }) {
            return Err(RegistryValidationError::InconsistentImplementationIdentity);
        }
    }

    Ok(())
}

fn validate_application_plan_catalog(
    application_plans: &[ApplicationPlan],
) -> Result<(), RegistryValidationError> {
    for (index, plan) in application_plans.iter().enumerate() {
        if plan.id.numeric() == 0 || plan.id.numeric() > APPLICATION_PLAN_COUNT {
            return Err(RegistryValidationError::UnknownApplicationPlanId);
        }
        if application_plans
            .iter()
            .skip(index + 1)
            .any(|other| other.id == plan.id)
        {
            return Err(RegistryValidationError::DuplicateApplicationPlanId);
        }
    }

    for expected in 1..=APPLICATION_PLAN_COUNT {
        if !application_plans
            .iter()
            .any(|plan| plan.id.numeric() == expected)
        {
            return Err(RegistryValidationError::MissingApplicationPlanId);
        }
    }

    Ok(())
}

fn valid_application_plan(descriptor: &SemanticDescriptor) -> bool {
    let whole_vector_at = |position: u16| {
        position
            .checked_sub(1)
            .and_then(|index| descriptor.parameters.get(usize::from(index)))
            .is_some_and(|operand| operand.consumption == OperandConsumption::WholeVector)
    };
    let result_valid = match descriptor.application_plan.result_cardinality {
        ResultCardinality::Elementwise => descriptor
            .parameters
            .iter()
            .all(|operand| operand.consumption == OperandConsumption::Elementwise),
        ResultCardinality::DynamicVector => true,
        ResultCardinality::Scalar => descriptor
            .parameters
            .iter()
            .any(|operand| operand.consumption == OperandConsumption::WholeVector),
        ResultCardinality::PreserveOperand(position)
        | ResultCardinality::OperandPlusOne(position) => whole_vector_at(position),
    };
    let work_valid = match descriptor.application_plan.resources.work {
        WorkAdmission::Constant(_) | WorkAdmission::ResultCardinality => true,
        WorkAdmission::OperandCardinality(position) => whole_vector_at(position),
    };
    result_valid && work_valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    struct FailingWriter {
        remaining: Option<usize>,
        bytes: Vec<u8>,
        fail_flush: bool,
    }

    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let Some(remaining) = self.remaining else {
                self.bytes.extend_from_slice(bytes);
                return Ok(bytes.len());
            };
            if remaining == 0 {
                return Err(io::Error::other("injected write failure"));
            }
            let count = remaining.min(bytes.len());
            self.bytes.extend_from_slice(&bytes[..count]);
            self.remaining = Some(remaining - count);
            Ok(count)
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                Err(io::Error::other("injected flush failure"))
            } else {
                Ok(())
            }
        }
    }

    fn registry_diagnostics(registry: &[SemanticDescriptor]) -> String {
        let mut output = Vec::new();
        write_registry_diagnostics(
            registry,
            &mut output,
            InternalRegistryDiagnosticFailureInjection::none(),
        )
        .expect("valid registry diagnostics");
        String::from_utf8(output).expect("diagnostics UTF-8")
    }

    #[test]
    fn internal_diagnostics_are_complete_grouped_and_stably_ordered() {
        let output = registry_diagnostics(SEMANTIC_REGISTRY);
        assert!(output.starts_with(
            "Faraweave semantic registry (internal human-readable diagnostics; format is unstable)\n"
        ));
        assert!(!output.contains('\r'));
        assert!(output.ends_with('\n'));

        let primitive_lines: Vec<&str> = output
            .lines()
            .filter(|line| line.starts_with("primitive "))
            .collect();
        let signature_lines: Vec<&str> = output
            .lines()
            .filter(|line| line.starts_with("  signature "))
            .collect();
        assert_eq!(primitive_lines.len(), usize::from(PRIMITIVE_COUNT));
        assert_eq!(signature_lines.len(), usize::from(SIGNATURE_COUNT));
        for primitive_id in 1..=PRIMITIVE_COUNT {
            let descriptor = SEMANTIC_REGISTRY
                .iter()
                .find(|descriptor| descriptor.primitive_id.numeric() == primitive_id)
                .expect("validated primitive");
            let expected = format!(
                "primitive id={primitive_id} name={} behavior={}",
                descriptor.primitive_name,
                structural_behavior_name(descriptor.behavior)
            );
            assert_eq!(
                primitive_lines
                    .iter()
                    .filter(|line| **line == expected)
                    .count(),
                1,
            );
        }
        let mut descriptors = SEMANTIC_REGISTRY.iter().collect::<Vec<_>>();
        descriptors.sort_unstable_by_key(|descriptor| descriptor.signature_id.numeric());
        for (descriptor, line) in descriptors.into_iter().zip(signature_lines) {
            let marker = format!(
                "  signature id={} implementation={} ",
                descriptor.signature_id.numeric(),
                descriptor.implementation_id.numeric()
            );
            assert!(line.starts_with(&marker));
            assert!(line.contains(&format!(" result={} ", descriptor.result.name())));
            assert!(line.contains(&format!(
                " application_plan={} ",
                descriptor.application_plan.id.numeric()
            )));
            assert!(line.ends_with(&format!(
                " kernel={}",
                scalar_kernel_name(descriptor.kernel)
            )));
            for (index, parameter) in descriptor.parameters.iter().enumerate() {
                assert!(line.contains(&format!(
                    "{}:accepted={},lift={}",
                    index + 1,
                    parameter.element_type.name(),
                    operand_consumption_name(parameter.consumption)
                )));
            }
            let mut cardinality = Vec::new();
            write_result_cardinality(
                &mut cardinality,
                descriptor.application_plan.result_cardinality,
            )
            .expect("cardinality diagnostic");
            let cardinality = String::from_utf8(cardinality).expect("cardinality UTF-8");
            assert!(line.contains(&format!(" result_cardinality={cardinality} ")));
            let mut work = Vec::new();
            write_work_admission(&mut work, descriptor.application_plan.resources.work)
                .expect("work diagnostic");
            let work = String::from_utf8(work).expect("work UTF-8");
            assert!(line.contains(&format!(" work={work} ")));
        }
        assert!(output.contains(
            "accepted=Double,lift=elementwise,actual={Int:promote_int_to_double,Double:identity}"
        ));
        assert!(output.contains(
            "primitive id=19 name=iota behavior=iota\n  signature id=34 implementation=34 \
             parameters=[1:accepted=Int,lift=elementwise,actual={Int:identity}] result=Int \
             application_plan=2 result_cardinality=dynamic_vector work=result_cardinality \
             kernel=iota_int\n"
        ));
        assert!(output.contains(
            "primitive id=21 name=length behavior=vector_length\n  signature id=37 \
             implementation=37 parameters=[1:accepted=Bool,lift=whole_vector,\
             actual={Bool:identity}] result=Int application_plan=3 result_cardinality=scalar \
             work=constant(1) kernel=length_bool_vector\n  signature id=38 implementation=38 \
             parameters=[1:accepted=Int,lift=whole_vector,actual={Int:identity}] result=Int \
             application_plan=3 result_cardinality=scalar work=constant(1) \
             kernel=length_int_vector\n  signature id=39 implementation=39 \
             parameters=[1:accepted=Double,lift=whole_vector,actual={Double:identity}] result=Int \
             application_plan=3 result_cardinality=scalar work=constant(1) \
             kernel=length_double_vector\n"
        ));
        assert!(
            output
                .lines()
                .flat_map(|line| line.split("; "))
                .all(|parameter| {
                    !parameter.contains("lift=whole_vector")
                        || !parameter.contains("promote_int_to_double")
                })
        );

        let mut reversed = SEMANTIC_REGISTRY.to_vec();
        reversed.reverse();
        assert_eq!(registry_diagnostics(&reversed), output);
    }

    #[test]
    fn internal_diagnostics_reject_empty_and_invalid_registries_before_output() {
        let mut empty_output = b"unchanged".to_vec();
        assert_eq!(
            write_registry_diagnostics(
                &[],
                &mut empty_output,
                InternalRegistryDiagnosticFailureInjection::none(),
            ),
            Err(InternalRegistryDiagnosticError::InvalidRegistry)
        );
        assert_eq!(empty_output, b"unchanged");

        let mut invalid = SEMANTIC_REGISTRY.to_vec();
        invalid[1].signature_id = invalid[0].signature_id;
        let mut invalid_output = b"unchanged".to_vec();
        assert_eq!(
            write_registry_diagnostics(
                &invalid,
                &mut invalid_output,
                InternalRegistryDiagnosticFailureInjection::none(),
            ),
            Err(InternalRegistryDiagnosticError::InvalidRegistry)
        );
        assert_eq!(invalid_output, b"unchanged");
    }

    #[test]
    fn internal_diagnostic_allocation_refusal_precedes_output() {
        let mut output = b"unchanged".to_vec();
        assert_eq!(
            write_registry_diagnostics(
                SEMANTIC_REGISTRY,
                &mut output,
                InternalRegistryDiagnosticFailureInjection::refuse_reservation(),
            ),
            Err(InternalRegistryDiagnosticError::AllocationUnavailable)
        );
        assert_eq!(output, b"unchanged");
    }

    #[test]
    fn internal_diagnostic_write_and_flush_failures_are_recoverable() {
        let mut write_failure = FailingWriter {
            remaining: Some(17),
            bytes: Vec::new(),
            fail_flush: false,
        };
        assert_eq!(
            write_internal_registry_diagnostics(&mut write_failure),
            Err(InternalRegistryDiagnosticError::WriteFailed)
        );
        assert_eq!(write_failure.bytes.len(), 17);

        let mut flush_failure = FailingWriter {
            remaining: None,
            bytes: Vec::new(),
            fail_flush: true,
        };
        assert_eq!(
            write_internal_registry_diagnostics(&mut flush_failure),
            Err(InternalRegistryDiagnosticError::FlushFailed)
        );
        assert!(!flush_failure.bytes.is_empty());

        let mut redirected = Vec::new();
        assert_eq!(write_internal_registry_diagnostics(&mut redirected), Ok(()));
        assert_eq!(
            String::from_utf8(redirected)
                .expect("redirected diagnostics UTF-8")
                .lines()
                .filter(|line| line.starts_with("primitive "))
                .count(),
            usize::from(PRIMITIVE_COUNT)
        );
    }

    #[test]
    fn production_registry_is_complete_and_numeric_lookups_are_checked() {
        assert_eq!(validate_registry(SEMANTIC_REGISTRY), Ok(()));
        let expected_primitives = [
            (1, "inc"),
            (1, "inc"),
            (2, "dec"),
            (2, "dec"),
            (3, "neg"),
            (3, "neg"),
            (4, "abs"),
            (4, "abs"),
            (5, "add"),
            (5, "add"),
            (6, "sub"),
            (6, "sub"),
            (7, "mul"),
            (7, "mul"),
            (8, "equals"),
            (8, "equals"),
            (8, "equals"),
            (9, "not_equals"),
            (9, "not_equals"),
            (9, "not_equals"),
            (10, "not"),
            (11, "and"),
            (12, "or"),
            (13, "odd"),
            (14, "even"),
            (15, "is_positive"),
            (15, "is_positive"),
            (16, "is_negative"),
            (16, "is_negative"),
            (17, "less_than"),
            (17, "less_than"),
            (18, "greater_than"),
            (18, "greater_than"),
            (19, "iota"),
            (20, "div"),
            (20, "div"),
            (21, "length"),
            (21, "length"),
            (21, "length"),
            (22, "sort"),
            (22, "sort"),
            (22, "sort"),
            (23, "sum"),
            (23, "sum"),
            (24, "all_of"),
            (25, "any_of"),
            (26, "none_of"),
            (27, "foldl"),
            (27, "foldl"),
            (27, "foldl"),
            (28, "scanl"),
            (28, "scanl"),
            (28, "scanl"),
            (29, "sqrt"),
            (30, "exp"),
            (31, "log"),
            (32, "log10"),
            (33, "sin"),
        ];
        assert_eq!(SEMANTIC_REGISTRY.len(), expected_primitives.len());
        for (index, (descriptor, expected)) in SEMANTIC_REGISTRY
            .iter()
            .zip(expected_primitives)
            .enumerate()
        {
            assert_eq!(
                (descriptor.primitive_id.numeric(), descriptor.primitive_name),
                expected
            );
            let stable_row_id = u16::try_from(index + 1).ok();
            assert_eq!(Some(descriptor.signature_id.numeric()), stable_row_id);
            assert_eq!(Some(descriptor.implementation_id.numeric()), stable_row_id);
        }
        assert_eq!(primitive_from_name("inc"), primitive_from_numeric(1));
        assert_eq!(primitive_from_name("div"), primitive_from_numeric(20));
        assert_eq!(primitive_from_name("length"), primitive_from_numeric(21));
        assert_eq!(primitive_from_name("sort"), primitive_from_numeric(22));
        assert_eq!(primitive_from_name("sum"), primitive_from_numeric(23));
        assert_eq!(primitive_from_name("all_of"), primitive_from_numeric(24));
        assert_eq!(primitive_from_name("any_of"), primitive_from_numeric(25));
        assert_eq!(primitive_from_name("none_of"), primitive_from_numeric(26));
        assert_eq!(primitive_from_name("foldl"), primitive_from_numeric(27));
        assert_eq!(primitive_from_name("scanl"), primitive_from_numeric(28));
        assert_eq!(
            signature_from_numeric(36).map(|descriptor| descriptor.primitive_name),
            Ok("div")
        );
        assert_eq!(
            implementation_from_numeric(36).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::DivDouble)
        );
        assert_eq!(
            signature_from_numeric(39).map(|descriptor| descriptor.primitive_name),
            Ok("length")
        );
        assert_eq!(
            implementation_from_numeric(39).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::LengthDoubleVector)
        );
        assert_eq!(
            signature_from_numeric(42).map(|descriptor| descriptor.primitive_name),
            Ok("sort")
        );
        assert_eq!(
            implementation_from_numeric(42).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::SortDoubleVector)
        );
        assert_eq!(
            signature_from_numeric(44).map(|descriptor| descriptor.primitive_name),
            Ok("sum")
        );
        assert_eq!(
            implementation_from_numeric(44).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::SumDoubleVector)
        );
        assert_eq!(
            signature_from_numeric(45).map(|descriptor| descriptor.primitive_name),
            Ok("all_of")
        );
        assert_eq!(
            implementation_from_numeric(45).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::AllOfBoolVector)
        );
        assert_eq!(
            signature_from_numeric(46).map(|descriptor| descriptor.primitive_name),
            Ok("any_of")
        );
        assert_eq!(
            implementation_from_numeric(46).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::AnyOfBoolVector)
        );
        assert_eq!(
            signature_from_numeric(47).map(|descriptor| descriptor.primitive_name),
            Ok("none_of")
        );
        assert_eq!(
            implementation_from_numeric(47).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::NoneOfBoolVector)
        );
        assert_eq!(
            signature_from_numeric(50).map(|descriptor| descriptor.primitive_name),
            Ok("foldl")
        );
        assert_eq!(
            implementation_from_numeric(50).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::FoldlDouble)
        );
        assert_eq!(
            signature_from_numeric(53).map(|descriptor| descriptor.primitive_name),
            Ok("scanl")
        );
        assert_eq!(
            implementation_from_numeric(53).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::ScanlDouble)
        );
        assert_eq!(
            implementation_from_numeric(34).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::IotaInt)
        );
        assert_eq!(application_plan_from_numeric(1), Ok(ELEMENTWISE_PLAN));
        assert_eq!(application_plan_from_numeric(2), Ok(IOTA_PLAN));
        assert_eq!(application_plan_from_numeric(3), Ok(LENGTH_PLAN));
        assert_eq!(application_plan_from_numeric(4), Ok(SORT_PLAN));
        assert_eq!(application_plan_from_numeric(5), Ok(SUM_PLAN));
        assert_eq!(application_plan_from_numeric(6), Ok(ALL_OF_PLAN));
        assert_eq!(application_plan_from_numeric(7), Ok(ANY_OF_PLAN));
        assert_eq!(application_plan_from_numeric(8), Ok(NONE_OF_PLAN));
        assert_eq!(application_plan_from_numeric(9), Ok(FOLDL_PLAN));
        assert_eq!(application_plan_from_numeric(10), Ok(SCANL_PLAN));
        assert!(
            SEMANTIC_REGISTRY
                .iter()
                .filter(|descriptor| descriptor.behavior == StructuralBehavior::Elementwise)
                .all(|descriptor| {
                    descriptor.application_plan == ELEMENTWISE_PLAN
                        && descriptor
                            .parameters
                            .iter()
                            .all(|operand| operand.consumption == OperandConsumption::Elementwise)
                })
        );
        assert!(
            SEMANTIC_REGISTRY
                .iter()
                .filter(|descriptor| descriptor.behavior == StructuralBehavior::Scanl)
                .all(|descriptor| {
                    descriptor.application_plan == SCANL_PLAN
                        && descriptor.parameters.len() == 2
                        && descriptor.parameters[0].element_type == descriptor.result
                        && descriptor.parameters[0].consumption == OperandConsumption::Elementwise
                        && descriptor.parameters[1].element_type == descriptor.result
                        && descriptor.parameters[1].consumption == OperandConsumption::WholeVector
                })
        );
        assert!(
            SEMANTIC_REGISTRY
                .iter()
                .filter(|descriptor| descriptor.behavior == StructuralBehavior::Foldl)
                .all(|descriptor| {
                    descriptor.application_plan == FOLDL_PLAN
                        && descriptor.parameters.len() == 2
                        && descriptor.parameters[0].element_type == descriptor.result
                        && descriptor.parameters[0].consumption == OperandConsumption::Elementwise
                        && descriptor.parameters[1].element_type == descriptor.result
                        && descriptor.parameters[1].consumption == OperandConsumption::WholeVector
                })
        );
        assert!(
            SEMANTIC_REGISTRY
                .iter()
                .filter(|descriptor| descriptor.behavior == StructuralBehavior::VectorSum)
                .all(|descriptor| {
                    descriptor.application_plan == SUM_PLAN
                        && descriptor.parameters.len() == 1
                        && descriptor.result == descriptor.parameters[0].element_type
                        && descriptor.parameters[0].consumption == OperandConsumption::WholeVector
                })
        );
        assert!(
            SEMANTIC_REGISTRY
                .iter()
                .filter(|descriptor| descriptor.behavior == StructuralBehavior::VectorAllOf)
                .all(|descriptor| {
                    descriptor.application_plan == ALL_OF_PLAN
                        && descriptor.parameters == WHOLE_BOOL1
                        && descriptor.result == ScalarType::Bool
                })
        );
        assert!(
            SEMANTIC_REGISTRY
                .iter()
                .filter(|descriptor| descriptor.behavior == StructuralBehavior::VectorAnyOf)
                .all(|descriptor| {
                    descriptor.application_plan == ANY_OF_PLAN
                        && descriptor.parameters == WHOLE_BOOL1
                        && descriptor.result == ScalarType::Bool
                })
        );
        assert!(
            SEMANTIC_REGISTRY
                .iter()
                .filter(|descriptor| descriptor.behavior == StructuralBehavior::VectorNoneOf)
                .all(|descriptor| {
                    descriptor.application_plan == NONE_OF_PLAN
                        && descriptor.parameters == WHOLE_BOOL1
                        && descriptor.result == ScalarType::Bool
                })
        );
        assert!(
            SEMANTIC_REGISTRY
                .iter()
                .filter(|descriptor| descriptor.behavior == StructuralBehavior::VectorSort)
                .all(|descriptor| {
                    descriptor.application_plan == SORT_PLAN
                        && descriptor.parameters.len() == 1
                        && descriptor.result == descriptor.parameters[0].element_type
                        && descriptor.parameters[0].consumption == OperandConsumption::WholeVector
                })
        );
        assert!(
            SEMANTIC_REGISTRY
                .iter()
                .filter(|descriptor| descriptor.behavior == StructuralBehavior::VectorLength)
                .all(|descriptor| {
                    descriptor.application_plan == LENGTH_PLAN
                        && descriptor.result == ScalarType::Int
                        && descriptor.parameters.len() == 1
                        && descriptor.parameters[0].consumption == OperandConsumption::WholeVector
                })
        );
        assert_eq!(
            signature_from_numeric(54).map(|descriptor| descriptor.primitive_name),
            Ok("sqrt")
        );
        assert_eq!(
            implementation_from_numeric(54).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::SqrtDouble)
        );
        assert_eq!(
            signature_from_numeric(55).map(|descriptor| descriptor.primitive_name),
            Ok("exp")
        );
        assert_eq!(
            implementation_from_numeric(55).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::ExpDouble)
        );
        assert_eq!(
            signature_from_numeric(56).map(|descriptor| descriptor.primitive_name),
            Ok("log")
        );
        assert_eq!(
            implementation_from_numeric(56).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::LogDouble)
        );
        assert_eq!(
            signature_from_numeric(57).map(|descriptor| descriptor.primitive_name),
            Ok("log10")
        );
        assert_eq!(
            implementation_from_numeric(57).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::Log10Double)
        );
        assert_eq!(
            signature_from_numeric(58).map(|descriptor| descriptor.primitive_name),
            Ok("sin")
        );
        assert_eq!(
            implementation_from_numeric(58).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::SinDouble)
        );
        assert_eq!(
            primitive_from_name("missing"),
            Err(RegistryLookupError::PrimitiveName)
        );
        assert_eq!(
            primitive_from_numeric(0),
            Err(RegistryLookupError::PrimitiveId)
        );
        assert_eq!(
            signature_from_numeric(59),
            Err(RegistryLookupError::SignatureId)
        );
        assert_eq!(
            implementation_from_numeric(59),
            Err(RegistryLookupError::ImplementationId)
        );
        assert_eq!(
            application_plan_from_numeric(11),
            Err(RegistryLookupError::ApplicationPlanId)
        );
    }

    #[test]
    fn backend_native_math_primitive_reservation_is_narrow() {
        assert!(!is_backend_native_math_primitive(28));
        for primitive_id in
            BACKEND_NATIVE_MATH_FIRST_PRIMITIVE_ID..=BACKEND_NATIVE_MATH_LAST_PRIMITIVE_ID
        {
            assert!(is_backend_native_math_primitive(primitive_id));
        }
        assert!(!is_backend_native_math_primitive(39));
    }

    #[test]
    fn invalid_registry_fixtures_reject_duplicate_missing_unknown_and_inconsistent_ids() {
        let mut duplicate = SEMANTIC_REGISTRY.to_vec();
        duplicate[1].signature_id = duplicate[0].signature_id;
        assert_eq!(
            validate_registry(&duplicate),
            Err(RegistryValidationError::DuplicateSignatureId)
        );
        let mut duplicate_implementation = SEMANTIC_REGISTRY.to_vec();
        duplicate_implementation[1].implementation_id =
            duplicate_implementation[0].implementation_id;
        assert_eq!(
            validate_registry(&duplicate_implementation),
            Err(RegistryValidationError::DuplicateImplementationId)
        );
        let mut duplicate_name = SEMANTIC_REGISTRY.to_vec();
        duplicate_name[2].primitive_name = duplicate_name[0].primitive_name;
        assert_eq!(
            validate_registry(&duplicate_name),
            Err(RegistryValidationError::DuplicatePrimitiveName)
        );

        let missing = &SEMANTIC_REGISTRY[..SEMANTIC_REGISTRY.len() - 3];
        assert_eq!(
            validate_registry(missing),
            Err(RegistryValidationError::MissingPrimitiveId)
        );

        let mut unknown = SEMANTIC_REGISTRY.to_vec();
        unknown[0].implementation_id = ImplementationId(IMPLEMENTATION_COUNT + 1);
        assert_eq!(
            validate_registry(&unknown),
            Err(RegistryValidationError::UnknownImplementationId)
        );
        unknown[0].implementation_id = SEMANTIC_REGISTRY[0].implementation_id;
        unknown[0].primitive_id = PrimitiveId(PRIMITIVE_COUNT + 1);
        assert_eq!(
            validate_registry(&unknown),
            Err(RegistryValidationError::UnknownPrimitiveId)
        );
        unknown[0].primitive_id = SEMANTIC_REGISTRY[0].primitive_id;
        unknown[0].signature_id = SignatureId(SIGNATURE_COUNT + 1);
        assert_eq!(
            validate_registry(&unknown),
            Err(RegistryValidationError::UnknownSignatureId)
        );
        unknown[0].signature_id = SEMANTIC_REGISTRY[0].signature_id;
        unknown[0].application_plan.id = ApplicationPlanId(APPLICATION_PLAN_COUNT + 1);
        assert_eq!(
            validate_registry(&unknown),
            Err(RegistryValidationError::UnknownApplicationPlanId)
        );

        let mut inconsistent = SEMANTIC_REGISTRY.to_vec();
        inconsistent[1].primitive_name = "increment";
        assert_eq!(
            validate_registry(&inconsistent),
            Err(RegistryValidationError::InconsistentPrimitiveIdentity)
        );
    }

    #[test]
    fn invalid_registry_fixtures_reject_changed_stable_semantic_meanings() {
        let mut swapped_primitives = SEMANTIC_REGISTRY.to_vec();
        for descriptor in &mut swapped_primitives[..2] {
            descriptor.primitive_name = "dec";
        }
        for descriptor in &mut swapped_primitives[2..4] {
            descriptor.primitive_name = "inc";
        }
        assert_eq!(
            validate_registry(&swapped_primitives),
            Err(RegistryValidationError::InconsistentPrimitiveIdentity)
        );

        let mut swapped_signatures = SEMANTIC_REGISTRY.to_vec();
        let first_signature = swapped_signatures[0].signature_id;
        swapped_signatures[0].signature_id = swapped_signatures[33].signature_id;
        swapped_signatures[33].signature_id = first_signature;
        assert_eq!(
            validate_registry(&swapped_signatures),
            Err(RegistryValidationError::InconsistentSignatureIdentity)
        );

        let mut swapped_implementations = SEMANTIC_REGISTRY.to_vec();
        let first_implementation = swapped_implementations[0].implementation_id;
        swapped_implementations[0].implementation_id =
            swapped_implementations[33].implementation_id;
        swapped_implementations[33].implementation_id = first_implementation;
        assert_eq!(
            validate_registry(&swapped_implementations),
            Err(RegistryValidationError::InconsistentImplementationIdentity)
        );

        let mut changed_signature_parameters = SEMANTIC_REGISTRY.to_vec();
        changed_signature_parameters[0].parameters = DOUBLE1;
        assert_eq!(
            validate_registry(&changed_signature_parameters),
            Err(RegistryValidationError::InconsistentSignatureIdentity)
        );

        const WHOLE_INT: &[OperandDescriptor] = &[OperandDescriptor {
            element_type: ScalarType::Int,
            consumption: OperandConsumption::WholeVector,
        }];
        let mut changed_consumption = SEMANTIC_REGISTRY.to_vec();
        changed_consumption[0].parameters = WHOLE_INT;
        assert_eq!(
            validate_registry(&changed_consumption),
            Err(RegistryValidationError::InvalidApplicationPlan)
        );

        let mut changed_signature_result = SEMANTIC_REGISTRY.to_vec();
        changed_signature_result[0].result = ScalarType::Double;
        assert_eq!(
            validate_registry(&changed_signature_result),
            Err(RegistryValidationError::InconsistentSignatureIdentity)
        );

        let mut changed_implementation_kernel = SEMANTIC_REGISTRY.to_vec();
        changed_implementation_kernel[0].kernel = ScalarKernel::DecInt;
        assert_eq!(
            validate_registry(&changed_implementation_kernel),
            Err(RegistryValidationError::InconsistentImplementationIdentity)
        );

        let mut changed_application_plan = SEMANTIC_REGISTRY.to_vec();
        changed_application_plan[0].application_plan = ApplicationPlan {
            id: ApplicationPlanId(1),
            result_cardinality: ResultCardinality::Scalar,
            resources: ResourceAdmissionPlan {
                work: WorkAdmission::Constant(1),
            },
        };
        assert_eq!(
            validate_registry(&changed_application_plan),
            Err(RegistryValidationError::InvalidApplicationPlan)
        );
    }

    #[test]
    fn application_plan_ids_reject_different_otherwise_valid_meanings() {
        const WHOLE_INT: &[OperandDescriptor] = &[OperandDescriptor {
            element_type: ScalarType::Int,
            consumption: OperandConsumption::WholeVector,
        }];
        let mut conflicting = SEMANTIC_REGISTRY.to_vec();
        conflicting[0].parameters = WHOLE_INT;
        conflicting[0].application_plan = ApplicationPlan {
            id: ApplicationPlanId(1),
            result_cardinality: ResultCardinality::Scalar,
            resources: ResourceAdmissionPlan {
                work: WorkAdmission::OperandCardinality(1),
            },
        };

        assert!(valid_application_plan(&conflicting[0]));
        assert_eq!(
            validate_registry(&conflicting),
            Err(RegistryValidationError::InconsistentApplicationPlanIdentity)
        );
    }

    #[test]
    fn application_plan_catalog_rejects_missing_and_duplicate_ids() {
        assert_eq!(
            validate_registry_with_application_plans(SEMANTIC_REGISTRY, &[ELEMENTWISE_PLAN]),
            Err(RegistryValidationError::MissingApplicationPlanId)
        );
        assert_eq!(
            validate_registry_with_application_plans(
                SEMANTIC_REGISTRY,
                &[
                    ELEMENTWISE_PLAN,
                    IOTA_PLAN,
                    LENGTH_PLAN,
                    SORT_PLAN,
                    SUM_PLAN,
                    ALL_OF_PLAN,
                    ANY_OF_PLAN,
                    NONE_OF_PLAN,
                    FOLDL_PLAN,
                    SCANL_PLAN,
                    IOTA_PLAN,
                ],
            ),
            Err(RegistryValidationError::DuplicateApplicationPlanId)
        );
    }

    #[test]
    fn container_plan_schema_validates_whole_vector_positions_and_resource_inputs() {
        const WHOLE_INT: &[OperandDescriptor] = &[OperandDescriptor {
            element_type: ScalarType::Int,
            consumption: OperandConsumption::WholeVector,
        }];
        let mut descriptor = SEMANTIC_REGISTRY[0];
        descriptor.parameters = WHOLE_INT;
        descriptor.application_plan = ApplicationPlan {
            id: ApplicationPlanId(2),
            result_cardinality: ResultCardinality::Scalar,
            resources: ResourceAdmissionPlan {
                work: WorkAdmission::OperandCardinality(1),
            },
        };
        assert!(valid_application_plan(&descriptor));

        descriptor.application_plan.result_cardinality = ResultCardinality::PreserveOperand(1);
        assert!(valid_application_plan(&descriptor));
        descriptor.application_plan.result_cardinality = ResultCardinality::OperandPlusOne(1);
        assert!(valid_application_plan(&descriptor));

        descriptor.application_plan.result_cardinality = ResultCardinality::PreserveOperand(2);
        assert!(!valid_application_plan(&descriptor));
        descriptor.application_plan.result_cardinality = ResultCardinality::Scalar;
        descriptor.application_plan.resources.work = WorkAdmission::OperandCardinality(2);
        assert!(!valid_application_plan(&descriptor));
    }

    #[test]
    fn conversion_table_accepts_only_identity_and_int_to_double() {
        assert_eq!(
            conversion(ScalarType::Bool, ScalarType::Bool),
            Some(Conversion::Identity)
        );
        assert_eq!(
            conversion(ScalarType::Int, ScalarType::Double),
            Some(Conversion::PromoteIntToDouble)
        );
        assert_eq!(conversion(ScalarType::Double, ScalarType::Int), None);
        assert_eq!(conversion(ScalarType::Bool, ScalarType::Int), None);
    }
}
