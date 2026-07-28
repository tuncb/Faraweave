use crate::ScalarType;

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

const PRIMITIVE_COUNT: u16 = 25;
const SIGNATURE_COUNT: u16 = 46;
const IMPLEMENTATION_COUNT: u16 = 46;
const APPLICATION_PLAN_COUNT: u16 = 7;

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

const APPLICATION_PLANS: &[ApplicationPlan] = &[
    ELEMENTWISE_PLAN,
    IOTA_PLAN,
    LENGTH_PLAN,
    SORT_PLAN,
    SUM_PLAN,
    ALL_OF_PLAN,
    ANY_OF_PLAN,
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
            primitive_from_name("missing"),
            Err(RegistryLookupError::PrimitiveName)
        );
        assert_eq!(
            primitive_from_numeric(0),
            Err(RegistryLookupError::PrimitiveId)
        );
        assert_eq!(
            signature_from_numeric(47),
            Err(RegistryLookupError::SignatureId)
        );
        assert_eq!(
            implementation_from_numeric(47),
            Err(RegistryLookupError::ImplementationId)
        );
        assert_eq!(
            application_plan_from_numeric(8),
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

        let missing = &SEMANTIC_REGISTRY[..SEMANTIC_REGISTRY.len() - 2];
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
