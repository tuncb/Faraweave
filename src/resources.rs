use crate::{Error, ErrorKind, ResourceErrorContext, ResourceErrorReason, SourceLocation, Value};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExecutionProfile {
    TrustedLocalV1,
    BoundedV1,
    #[default]
    TrustedLocalV2,
    BoundedV2,
}

impl ExecutionProfile {
    pub const fn name(self) -> &'static str {
        match self {
            Self::TrustedLocalV1 => "trusted-local-v1",
            Self::BoundedV1 => "bounded-v1",
            Self::TrustedLocalV2 => "trusted-local-v2",
            Self::BoundedV2 => "bounded-v2",
        }
    }

    pub const fn supports_tuples(self) -> bool {
        matches!(self, Self::TrustedLocalV2 | Self::BoundedV2)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceLimits {
    pub max_vector_bytes: Option<usize>,
    pub max_tuple_table_bytes: Option<usize>,
    pub max_live_evaluation_bytes: Option<usize>,
    pub max_work_units: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AllocationFailureInjection {
    pub fail_at_ordinal: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceUsage {
    pub live_evaluation_bytes: usize,
    pub peak_live_evaluation_bytes: usize,
    pub work_units: usize,
    pub allocation_attempts: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceEventKind {
    Admission,
    Refusal,
    Release,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceEvent<'a> {
    pub kind: ResourceEventKind,
    pub producer: &'a str,
    pub requested_elements: Option<usize>,
    pub requested_bytes: Option<usize>,
    pub requested_work_units: usize,
    pub allocation_ordinal: Option<usize>,
    pub refusal_reason: Option<ResourceErrorReason>,
    pub usage: ResourceUsage,
}

pub type ResourceObserver = for<'a> fn(&ResourceEvent<'a>);

#[derive(Debug)]
pub(crate) struct ResourceContext {
    profile: ExecutionProfile,
    limits: ResourceLimits,
    injection: AllocationFailureInjection,
    observer: Option<ResourceObserver>,
    pub usage: ResourceUsage,
}

impl ResourceContext {
    pub(crate) fn new(
        profile: ExecutionProfile,
        limits: ResourceLimits,
        injection: AllocationFailureInjection,
    ) -> Result<Self, Error> {
        Self::new_with_observer(profile, limits, injection, None)
    }

    pub(crate) fn new_with_observer(
        profile: ExecutionProfile,
        limits: ResourceLimits,
        injection: AllocationFailureInjection,
        observer: Option<ResourceObserver>,
    ) -> Result<Self, Error> {
        let trusted = matches!(
            profile,
            ExecutionProfile::TrustedLocalV1 | ExecutionProfile::TrustedLocalV2
        );
        let any_limit = limits.max_vector_bytes.is_some()
            || limits.max_tuple_table_bytes.is_some()
            || limits.max_live_evaluation_bytes.is_some()
            || limits.max_work_units.is_some();
        if trusted && any_limit {
            return Err(Error::new(
                ErrorKind::InvalidExecutionProfile,
                SourceLocation::start(),
                format!(
                    "{} requires every resource limit to be omitted",
                    profile.name()
                ),
            ));
        }
        if !trusted && !any_limit {
            return Err(Error::new(
                ErrorKind::InvalidExecutionProfile,
                SourceLocation::start(),
                format!(
                    "{} requires at least one configured resource limit",
                    profile.name()
                ),
            ));
        }
        if matches!(profile, ExecutionProfile::BoundedV1) && limits.max_tuple_table_bytes.is_some()
        {
            return Err(Error::new(
                ErrorKind::InvalidExecutionProfile,
                SourceLocation::start(),
                "bounded-v1 does not accept max_tuple_table_bytes",
            ));
        }
        Ok(Self {
            profile,
            limits,
            injection,
            observer,
            usage: ResourceUsage::default(),
        })
    }

    pub(crate) fn require_tuple_profile(&self, location: SourceLocation) -> Result<(), Error> {
        if self.profile.supports_tuples() {
            return Ok(());
        }
        Err(Error::new(
            ErrorKind::ProfileError,
            location,
            format!("{} does not support value kind Tuple", self.profile.name()),
        ))
    }

    pub(crate) fn charge_work(
        &mut self,
        amount: usize,
        location: SourceLocation,
        producer: &str,
    ) -> Result<(), Error> {
        self.admit_request(None, 0, amount, Some(amount), None, location, producer)
    }

    pub(crate) fn size_overflow(
        &self,
        requested_elements: Option<usize>,
        location: SourceLocation,
        producer: &str,
    ) -> Error {
        self.refusal(
            ResourceErrorReason::SizeOverflow,
            location,
            producer,
            requested_elements,
            None,
            0,
            None,
            None,
            None,
            None,
        )
    }

    pub(crate) fn admit_vector(
        &mut self,
        element_type: crate::ScalarType,
        length: usize,
        location: SourceLocation,
        producer: &str,
    ) -> Result<usize, Error> {
        self.admit_vector_with_work(element_type, length, 0, location, producer)
    }

    pub(crate) fn admit_vector_with_work(
        &mut self,
        element_type: crate::ScalarType,
        length: usize,
        work: usize,
        location: SourceLocation,
        producer: &str,
    ) -> Result<usize, Error> {
        let bytes = match length.checked_mul(element_type.byte_width()) {
            Some(bytes) => bytes,
            None => {
                return Err(self.refusal(
                    ResourceErrorReason::SizeOverflow,
                    location,
                    producer,
                    Some(length),
                    None,
                    work,
                    None,
                    None,
                    None,
                    None,
                ));
            }
        };
        self.admit_request(
            self.limits
                .max_vector_bytes
                .map(|limit| ("max_vector_bytes", limit)),
            bytes,
            work,
            Some(length),
            Some(bytes),
            location,
            producer,
        )?;
        Ok(bytes)
    }

    pub(crate) fn admit_string(
        &mut self,
        length: usize,
        work: usize,
        location: SourceLocation,
        producer: &str,
    ) -> Result<usize, Error> {
        self.admit_request(None, length, work, None, Some(length), location, producer)?;
        Ok(length)
    }

    pub(crate) fn admit_string_vector(
        &mut self,
        length: usize,
        payload_bytes: usize,
        work: usize,
        location: SourceLocation,
        producer: &str,
    ) -> Result<usize, Error> {
        let bytes = length
            .checked_mul(16)
            .and_then(|descriptors| descriptors.checked_add(payload_bytes))
            .ok_or_else(|| {
                self.refusal(
                    ResourceErrorReason::SizeOverflow,
                    location,
                    producer,
                    Some(length),
                    None,
                    work,
                    None,
                    None,
                    None,
                    None,
                )
            })?;
        self.admit_request(
            self.limits
                .max_vector_bytes
                .map(|limit| ("max_vector_bytes", limit)),
            bytes,
            work,
            Some(length),
            Some(bytes),
            location,
            producer,
        )?;
        Ok(bytes)
    }

    pub(crate) fn admit_tuple(
        &mut self,
        count: usize,
        location: SourceLocation,
        producer: &str,
    ) -> Result<usize, Error> {
        self.require_tuple_profile(location)?;
        let bytes = match count.checked_mul(16) {
            Some(bytes) => bytes,
            None => {
                return Err(self.refusal(
                    ResourceErrorReason::SizeOverflow,
                    location,
                    producer,
                    Some(count),
                    None,
                    0,
                    None,
                    None,
                    None,
                    None,
                ));
            }
        };
        self.admit_request(
            self.limits
                .max_tuple_table_bytes
                .map(|limit| ("max_tuple_table_bytes", limit)),
            bytes,
            0,
            Some(count),
            Some(bytes),
            location,
            producer,
        )?;
        Ok(bytes)
    }

    #[allow(clippy::too_many_arguments)]
    fn admit_request(
        &mut self,
        request_limit: Option<(&'static str, usize)>,
        bytes: usize,
        work: usize,
        requested_elements: Option<usize>,
        requested_bytes: Option<usize>,
        location: SourceLocation,
        producer: &str,
    ) -> Result<(), Error> {
        let live_after = match self.usage.live_evaluation_bytes.checked_add(bytes) {
            Some(value) => value,
            None => {
                return Err(self.refusal(
                    ResourceErrorReason::SizeOverflow,
                    location,
                    producer,
                    requested_elements,
                    requested_bytes,
                    work,
                    None,
                    None,
                    None,
                    None,
                ));
            }
        };
        let work_after = match self.usage.work_units.checked_add(work) {
            Some(value) => value,
            None => {
                return Err(self.refusal(
                    ResourceErrorReason::SizeOverflow,
                    location,
                    producer,
                    requested_elements,
                    requested_bytes,
                    work,
                    None,
                    None,
                    None,
                    None,
                ));
            }
        };
        if let Some((kind, limit)) = request_limit
            && bytes > limit
        {
            return Err(self.refusal(
                ResourceErrorReason::ProfileLimit,
                location,
                producer,
                requested_elements,
                requested_bytes,
                work,
                Some(kind),
                Some(limit),
                Some(0),
                Some(bytes),
            ));
        }
        if let Some(limit) = self.limits.max_live_evaluation_bytes
            && live_after > limit
        {
            return Err(self.refusal(
                ResourceErrorReason::ProfileLimit,
                location,
                producer,
                requested_elements,
                requested_bytes,
                work,
                Some("max_live_evaluation_bytes"),
                Some(limit),
                Some(self.usage.live_evaluation_bytes),
                Some(bytes),
            ));
        }
        if let Some(limit) = self.limits.max_work_units
            && work_after > limit
        {
            return Err(self.refusal(
                ResourceErrorReason::ProfileLimit,
                location,
                producer,
                requested_elements,
                requested_bytes,
                work,
                Some("max_work_units"),
                Some(limit),
                Some(self.usage.work_units),
                Some(work),
            ));
        }
        if bytes != 0 {
            let ordinal = self.usage.allocation_attempts;
            self.usage.allocation_attempts = match ordinal.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(self.refusal(
                        ResourceErrorReason::SizeOverflow,
                        location,
                        producer,
                        requested_elements,
                        requested_bytes,
                        work,
                        None,
                        None,
                        None,
                        None,
                    ));
                }
            };
            if self.injection.fail_at_ordinal == Some(ordinal) {
                return Err(self.refusal(
                    ResourceErrorReason::AllocationUnavailable,
                    location,
                    producer,
                    requested_elements,
                    requested_bytes,
                    work,
                    None,
                    None,
                    None,
                    Some(bytes),
                ));
            }
        }
        self.usage.live_evaluation_bytes = live_after;
        self.usage.work_units = work_after;
        self.usage.peak_live_evaluation_bytes =
            self.usage.peak_live_evaluation_bytes.max(live_after);
        self.notify(ResourceEvent {
            kind: ResourceEventKind::Admission,
            producer,
            requested_elements,
            requested_bytes,
            requested_work_units: work,
            allocation_ordinal: (bytes != 0)
                .then_some(self.usage.allocation_attempts.saturating_sub(1)),
            refusal_reason: None,
            usage: self.usage,
        });
        Ok(())
    }

    pub(crate) fn release_owned(&mut self, value: Value) -> Result<(), Error> {
        let bytes = value.into_canonical_bytes()?;
        self.release_bytes(bytes);
        Ok(())
    }

    pub(crate) fn release_bytes(&mut self, bytes: usize) {
        self.refund(bytes);
        self.notify(ResourceEvent {
            kind: ResourceEventKind::Release,
            producer: "value_release",
            requested_elements: None,
            requested_bytes: Some(bytes),
            requested_work_units: 0,
            allocation_ordinal: None,
            refusal_reason: None,
            usage: self.usage,
        });
    }

    pub(crate) fn refund(&mut self, bytes: usize) {
        self.usage.live_evaluation_bytes = self.usage.live_evaluation_bytes.saturating_sub(bytes);
    }

    #[allow(clippy::too_many_arguments)]
    fn resource_error(
        &self,
        reason: ResourceErrorReason,
        location: SourceLocation,
        producer: &str,
        requested_elements: Option<usize>,
        requested_bytes: Option<usize>,
        limit_kind: Option<&'static str>,
        limit: Option<usize>,
        usage_before: Option<usize>,
        refused_charge: Option<usize>,
    ) -> Error {
        let reason_name = match reason {
            ResourceErrorReason::SizeOverflow => "size_overflow",
            ResourceErrorReason::ProfileLimit => "profile_limit",
            ResourceErrorReason::AllocationUnavailable => "allocation_unavailable",
        };
        let mut error = Error::new(
            ErrorKind::ResourceError,
            location,
            format!("{producer} resource request failed: {reason_name}"),
        );
        error.primitive = Some(producer.to_owned());
        error.resource = Some(ResourceErrorContext {
            reason,
            requested_elements,
            requested_bytes,
            profile: self.profile.name(),
            limit_kind,
            configured_limit: limit,
            usage_before,
            refused_charge,
            allocation_ordinal: (reason == ResourceErrorReason::AllocationUnavailable)
                .then_some(self.usage.allocation_attempts.saturating_sub(1)),
        });
        error
    }

    #[allow(clippy::too_many_arguments)]
    fn refusal(
        &self,
        reason: ResourceErrorReason,
        location: SourceLocation,
        producer: &str,
        requested_elements: Option<usize>,
        requested_bytes: Option<usize>,
        requested_work_units: usize,
        limit_kind: Option<&'static str>,
        limit: Option<usize>,
        usage_before: Option<usize>,
        refused_charge: Option<usize>,
    ) -> Error {
        self.notify(ResourceEvent {
            kind: ResourceEventKind::Refusal,
            producer,
            requested_elements,
            requested_bytes,
            requested_work_units,
            allocation_ordinal: (reason == ResourceErrorReason::AllocationUnavailable)
                .then_some(self.usage.allocation_attempts.saturating_sub(1)),
            refusal_reason: Some(reason),
            usage: self.usage,
        });
        self.resource_error(
            reason,
            location,
            producer,
            requested_elements,
            requested_bytes,
            limit_kind,
            limit,
            usage_before,
            refused_charge,
        )
    }

    fn notify(&self, event: ResourceEvent<'_>) {
        if let Some(observer) = self.observer {
            observer(&event);
        }
    }
}
