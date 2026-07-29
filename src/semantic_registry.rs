use crate::ScalarType;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PrimitiveId(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SignatureId(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ImplementationId(u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Conversion {
    Identity,
    PromoteIntToDouble,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StructuralBehavior {
    Elementwise,
    Iota,
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
    pub parameters: &'static [ScalarType],
    pub result: ScalarType,
    pub behavior: StructuralBehavior,
    pub kernel: ScalarKernel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum RegistryLookupError {
    PrimitiveName,
    PrimitiveId,
    SignatureId,
    ImplementationId,
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
    InconsistentPrimitiveIdentity,
    InconsistentSignatureIdentity,
    InconsistentImplementationIdentity,
}

const SIGNATURE_COUNT: u16 = 39;
const IMPLEMENTATION_COUNT: u16 = 39;

pub(crate) const BACKEND_NATIVE_MATH_FIRST_PRIMITIVE_ID: u16 = 29;
pub(crate) const BACKEND_NATIVE_MATH_LAST_PRIMITIVE_ID: u16 = 38;

pub(crate) const fn is_backend_native_math_primitive(primitive_id: u16) -> bool {
    primitive_id >= BACKEND_NATIVE_MATH_FIRST_PRIMITIVE_ID
        && primitive_id <= BACKEND_NATIVE_MATH_LAST_PRIMITIVE_ID
}

const INT1: &[ScalarType] = &[ScalarType::Int];
const DOUBLE1: &[ScalarType] = &[ScalarType::Double];
const BOOL1: &[ScalarType] = &[ScalarType::Bool];
const INT2: &[ScalarType] = &[ScalarType::Int, ScalarType::Int];
const DOUBLE2: &[ScalarType] = &[ScalarType::Double, ScalarType::Double];
const BOOL2: &[ScalarType] = &[ScalarType::Bool, ScalarType::Bool];

macro_rules! descriptor {
    ($primitive:literal, $name:literal, $signature:literal, $implementation:literal,
     $parameters:ident, $result:ident, $behavior:ident, $kernel:ident) => {
        SemanticDescriptor {
            primitive_id: PrimitiveId($primitive),
            primitive_name: $name,
            signature_id: SignatureId($signature),
            implementation_id: ImplementationId($implementation),
            parameters: $parameters,
            result: ScalarType::$result,
            behavior: StructuralBehavior::$behavior,
            kernel: ScalarKernel::$kernel,
        }
    };
}

// This is the single production owner of primitive names and all stable semantic IDs.
pub(crate) const SEMANTIC_REGISTRY: &[SemanticDescriptor] = &[
    descriptor!(1, "inc", 1, 1, INT1, Int, Elementwise, IncInt),
    descriptor!(1, "inc", 2, 2, DOUBLE1, Double, Elementwise, IncDouble),
    descriptor!(2, "dec", 3, 3, INT1, Int, Elementwise, DecInt),
    descriptor!(2, "dec", 4, 4, DOUBLE1, Double, Elementwise, DecDouble),
    descriptor!(3, "neg", 5, 5, INT1, Int, Elementwise, NegInt),
    descriptor!(3, "neg", 6, 6, DOUBLE1, Double, Elementwise, NegDouble),
    descriptor!(4, "abs", 7, 7, INT1, Int, Elementwise, AbsInt),
    descriptor!(4, "abs", 8, 8, DOUBLE1, Double, Elementwise, AbsDouble),
    descriptor!(5, "add", 9, 9, INT2, Int, Elementwise, AddInt),
    descriptor!(5, "add", 10, 10, DOUBLE2, Double, Elementwise, AddDouble),
    descriptor!(6, "sub", 11, 11, INT2, Int, Elementwise, SubInt),
    descriptor!(6, "sub", 12, 12, DOUBLE2, Double, Elementwise, SubDouble),
    descriptor!(7, "mul", 13, 13, INT2, Int, Elementwise, MulInt),
    descriptor!(7, "mul", 14, 14, DOUBLE2, Double, Elementwise, MulDouble),
    descriptor!(8, "equals", 15, 15, BOOL2, Bool, Elementwise, EqualsBool),
    descriptor!(8, "equals", 16, 16, INT2, Bool, Elementwise, EqualsInt),
    descriptor!(
        8,
        "equals",
        17,
        17,
        DOUBLE2,
        Bool,
        Elementwise,
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
        NotEqualsDouble
    ),
    descriptor!(10, "not", 21, 21, BOOL1, Bool, Elementwise, NotBool),
    descriptor!(11, "and", 22, 22, BOOL2, Bool, Elementwise, AndBool),
    descriptor!(12, "or", 23, 23, BOOL2, Bool, Elementwise, OrBool),
    descriptor!(13, "odd", 24, 24, INT1, Bool, Elementwise, OddInt),
    descriptor!(14, "even", 25, 25, INT1, Bool, Elementwise, EvenInt),
    descriptor!(
        15,
        "is_positive",
        26,
        26,
        INT1,
        Bool,
        Elementwise,
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
        GreaterThanDouble
    ),
    descriptor!(19, "iota", 34, 34, INT1, Int, Iota, IotaInt),
    descriptor!(29, "sqrt", 35, 35, DOUBLE1, Double, Elementwise, SqrtDouble),
    descriptor!(30, "exp", 36, 36, DOUBLE1, Double, Elementwise, ExpDouble),
    descriptor!(31, "log", 37, 37, DOUBLE1, Double, Elementwise, LogDouble),
    descriptor!(
        32,
        "log10",
        38,
        38,
        DOUBLE1,
        Double,
        Elementwise,
        Log10Double
    ),
    descriptor!(33, "sin", 39, 39, DOUBLE1, Double, Elementwise, SinDouble),
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
    for descriptor in registry {
        if primitive_from_numeric(descriptor.primitive_id.numeric()).is_err() {
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

    for expected in SEMANTIC_REGISTRY {
        if !registry
            .iter()
            .any(|descriptor| descriptor.primitive_id == expected.primitive_id)
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
                || descriptor.kernel != canonical.kernel
        }) {
            return Err(RegistryValidationError::InconsistentImplementationIdentity);
        }
    }

    Ok(())
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
        assert_eq!(
            signature_from_numeric(34).map(|descriptor| descriptor.primitive_name),
            Ok("iota")
        );
        assert_eq!(
            implementation_from_numeric(34).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::IotaInt)
        );
        assert_eq!(
            signature_from_numeric(35).map(|descriptor| descriptor.primitive_name),
            Ok("sqrt")
        );
        assert_eq!(
            implementation_from_numeric(35).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::SqrtDouble)
        );
        assert_eq!(
            signature_from_numeric(36).map(|descriptor| descriptor.primitive_name),
            Ok("exp")
        );
        assert_eq!(
            implementation_from_numeric(36).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::ExpDouble)
        );
        assert_eq!(
            signature_from_numeric(37).map(|descriptor| descriptor.primitive_name),
            Ok("log")
        );
        assert_eq!(
            implementation_from_numeric(37).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::LogDouble)
        );
        assert_eq!(
            signature_from_numeric(38).map(|descriptor| descriptor.primitive_name),
            Ok("log10")
        );
        assert_eq!(
            implementation_from_numeric(38).map(|descriptor| descriptor.kernel),
            Ok(ScalarKernel::Log10Double)
        );
        assert_eq!(
            signature_from_numeric(39).map(|descriptor| descriptor.primitive_name),
            Ok("sin")
        );
        assert_eq!(
            implementation_from_numeric(39).map(|descriptor| descriptor.kernel),
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
            signature_from_numeric(40),
            Err(RegistryLookupError::SignatureId)
        );
        assert_eq!(
            implementation_from_numeric(40),
            Err(RegistryLookupError::ImplementationId)
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

        let missing = &SEMANTIC_REGISTRY[..SEMANTIC_REGISTRY.len() - 1];
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
        unknown[0].primitive_id = PrimitiveId(20);
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
