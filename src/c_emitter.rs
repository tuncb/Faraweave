use crate::lowering::compile_parsed_source;
use crate::parser::{
    first_tuple_location, parse, program_contains_tuple, validate_parameter_declarations,
};
use crate::primitive::resolve_names;
use crate::semantic_registry::{SEMANTIC_REGISTRY, ScalarKernel, implementation_from_numeric};
use crate::typed_program::{
    ConstantRecord, Conversion as IrConversion, Feature, IndexRange, LiftMode, Node, NodeKind,
    Origin, RawProgram, ScalarConstant, TypeRecord, ValueAccess, VerifiedProgram,
};
use crate::{Error, ErrorKind, EvaluationConfiguration, ScalarType};
use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub struct CEmissionResult {
    pub source: String,
}

pub(crate) fn emit_verified_c_program(
    program: &VerifiedProgram,
    configuration: EvaluationConfiguration,
) -> Result<CEmissionResult, Error> {
    validate_ir_emission_configuration(program.as_raw(), configuration)?;
    IrCGenerator::new(program.as_raw(), configuration).emit()
}

fn validate_ir_emission_configuration(
    program: &RawProgram,
    configuration: EvaluationConfiguration,
) -> Result<(), Error> {
    let resources = crate::resources::ResourceContext::new(
        configuration.profile,
        configuration.limits,
        configuration.allocation_failure,
    )?;
    if !program.features.contains(&Feature::Tuples.numeric()) {
        return Ok(());
    }
    let location = program
        .nodes
        .iter()
        .filter(|node| {
            program
                .types
                .get(node.result_type.0 as usize)
                .is_some_and(|record| matches!(record, TypeRecord::Tuple { .. }))
        })
        .filter_map(|node| program.origins.get(node.origin.0 as usize))
        .min_by_key(|origin| origin.span.begin.offset)
        .map(|origin| crate::SourceLocation {
            offset: origin.span.begin.offset as usize,
            line: origin.span.begin.line as usize,
            column: origin.span.begin.column as usize,
        })
        .unwrap_or_else(crate::SourceLocation::start);
    resources.require_tuple_profile(location)
}

struct IrCGenerator<'a> {
    program: &'a RawProgram,
    configuration: EvaluationConfiguration,
    definitions: String,
}

impl<'a> IrCGenerator<'a> {
    fn new(program: &'a RawProgram, configuration: EvaluationConfiguration) -> Self {
        Self {
            program,
            configuration,
            definitions: String::new(),
        }
    }

    fn emit(mut self) -> Result<CEmissionResult, Error> {
        self.emit_selected_implementation_wrappers()?;
        for (index, node) in self.program.nodes.iter().copied().enumerate() {
            self.emit_node(index, node)?;
        }

        let mut source = parameter_runtime()?;
        source.push_str("\n/* VerifiedProgram-driven definitions. */\n");
        source.push_str("FWV fw_parameters[");
        write!(source, "{}", self.program.parameters.len().max(1)).map_err(|_| emission_error())?;
        source.push_str("];\nconst int fw_parameter_types[] = {");
        for parameter in &self.program.parameters {
            write!(source, "{},", scalar_tag(parameter.scalar_type))
                .map_err(|_| emission_error())?;
        }
        source.push_str("0};\nconst char *const fw_parameter_names[] = {");
        for parameter in &self.program.parameters {
            write!(source, "{},", c_string(&parameter.name)).map_err(|_| emission_error())?;
        }
        source.push_str("\"\"};\nconst size_t fw_parameter_spans[][6] = {");
        for parameter in &self.program.parameters {
            let origin = self.origin(parameter.declaration_origin.0)?;
            write!(
                source,
                "{{{}U,{}U,{}U,{}U,{}U,{}U}},",
                origin.span.begin.offset,
                origin.span.begin.line,
                origin.span.begin.column,
                origin.span.end.offset,
                origin.span.end.line,
                origin.span.end.column
            )
            .map_err(|_| emission_error())?;
        }
        source.push_str("{0U,0U,0U,0U,0U,0U}};\n");
        source.push_str(&self.definitions);
        self.emit_configuration(&mut source)?;
        source.push_str("static const FWExpr fw_roots[] = {");
        for root in &self.program.roots {
            write!(source, "fw_ir_node_{},", root.node.0).map_err(|_| emission_error())?;
        }
        source.push_str("NULL};\n");
        writeln!(
            source,
            "int main(int argc, char **argv) {{ return fw_main(argc, argv, {}U, fw_roots); }}",
            self.program.roots.len()
        )
        .map_err(|_| emission_error())?;
        Ok(CEmissionResult { source })
    }

    fn emit_configuration(&self, source: &mut String) -> Result<(), Error> {
        writeln!(
            source,
            "const size_t fw_required = {}U;\n\
             const int fw_profile = {};\n\
             const int fw_has_vector_limit = {};\n\
             const size_t fw_vector_limit = {}U;\n\
             const int fw_has_tuple_limit = {};\n\
             const size_t fw_tuple_limit = {}U;\n\
             const int fw_has_live_limit = {};\n\
             const size_t fw_live_limit = {}U;\n\
             const int fw_has_work_limit = {};\n\
             const size_t fw_work_limit = {}U;\n\
             const int fw_has_failure_ordinal = {};\n\
             const size_t fw_failure_ordinal = {}U;",
            self.program.parameters.len(),
            match self.configuration.profile {
                crate::ExecutionProfile::TrustedLocalV1 => 0,
                crate::ExecutionProfile::BoundedV1 => 1,
                crate::ExecutionProfile::TrustedLocalV2 => 2,
                crate::ExecutionProfile::BoundedV2 => 3,
            },
            i32::from(self.configuration.limits.max_vector_bytes.is_some()),
            self.configuration.limits.max_vector_bytes.unwrap_or(0),
            i32::from(self.configuration.limits.max_tuple_table_bytes.is_some()),
            self.configuration.limits.max_tuple_table_bytes.unwrap_or(0),
            i32::from(
                self.configuration
                    .limits
                    .max_live_evaluation_bytes
                    .is_some()
            ),
            self.configuration
                .limits
                .max_live_evaluation_bytes
                .unwrap_or(0),
            i32::from(self.configuration.limits.max_work_units.is_some()),
            self.configuration.limits.max_work_units.unwrap_or(0),
            i32::from(
                self.configuration
                    .allocation_failure
                    .fail_at_ordinal
                    .is_some()
            ),
            self.configuration
                .allocation_failure
                .fail_at_ordinal
                .unwrap_or(0),
        )
        .map_err(|_| emission_error())
    }

    fn emit_selected_implementation_wrappers(&mut self) -> Result<(), Error> {
        for descriptor in SEMANTIC_REGISTRY {
            let used = self.program.nodes.iter().any(|node| {
                matches!(
                    node.kind,
                    NodeKind::SelectedApply {
                        implementation_id,
                        ..
                    } if implementation_id == descriptor.implementation_id.numeric()
                )
            });
            if !used {
                continue;
            }
            let implementation = descriptor.implementation_id.numeric();
            if descriptor.kernel == ScalarKernel::IotaInt {
                writeln!(
                    self.definitions,
                    "static int fw_impl_{implementation}(const FWV *args,size_t count,FWV *out,size_t line,size_t column,const size_t (*origins)[2],size_t origin_count,size_t static_anchor,const size_t *shape_checks,size_t shape_count,const int *conversions,int lift) {{ (void)count;(void)origins;(void)origin_count;(void)static_anchor;(void)shape_checks;(void)shape_count;(void)conversions;(void)lift; return fw_apply_selected_iota({},args,out,line,column); }}",
                    c_string(descriptor.primitive_name),
                )
                .map_err(|_| emission_error())?;
                continue;
            }
            self.emit_selected_kernel(implementation, descriptor.kernel)?;
            writeln!(
                self.definitions,
                "static int fw_impl_{implementation}(const FWV *args,size_t count,FWV *out,size_t line,size_t column,const size_t (*origins)[2],size_t origin_count,size_t static_anchor,const size_t *shape_checks,size_t shape_count,const int *conversions,int lift) {{ return fw_apply_selected(fw_kernel_{implementation}, {}, {}, args, count, out, line, column, origins, origin_count, static_anchor, shape_checks, shape_count, conversions, lift); }}",
                c_string(descriptor.primitive_name),
                scalar_tag(descriptor.result),
            )
            .map_err(|_| emission_error())?;
        }
        Ok(())
    }

    fn emit_selected_kernel(
        &mut self,
        implementation: u16,
        kernel: ScalarKernel,
    ) -> Result<(), Error> {
        let body = match kernel {
            ScalarKernel::IncInt => {
                "if(args[0].i==INT64_MAX)return fw_selected_integer_overflow(name,line,column,index,vector_result);fw_set_int(out,args[0].i+INT64_C(1));return 1;"
            }
            ScalarKernel::DecInt => {
                "if(args[0].i==INT64_MIN)return fw_selected_integer_overflow(name,line,column,index,vector_result);fw_set_int(out,args[0].i-INT64_C(1));return 1;"
            }
            ScalarKernel::NegInt => {
                "if(args[0].i==INT64_MIN)return fw_selected_integer_overflow(name,line,column,index,vector_result);fw_set_int(out,-args[0].i);return 1;"
            }
            ScalarKernel::AbsInt => {
                "if(args[0].i==INT64_MIN)return fw_selected_integer_overflow(name,line,column,index,vector_result);fw_set_int(out,args[0].i<0?-args[0].i:args[0].i);return 1;"
            }
            ScalarKernel::AddInt => {
                "int64_t a=args[0].i,b=args[1].i;if((b>0&&a>INT64_MAX-b)||(b<0&&a<INT64_MIN-b))return fw_selected_integer_overflow(name,line,column,index,vector_result);fw_set_int(out,a+b);return 1;"
            }
            ScalarKernel::SubInt => {
                "int64_t a=args[0].i,b=args[1].i;if((b<0&&a>INT64_MAX+b)||(b>0&&a<INT64_MIN+b))return fw_selected_integer_overflow(name,line,column,index,vector_result);fw_set_int(out,a-b);return 1;"
            }
            ScalarKernel::MulInt => {
                "int64_t a=args[0].i,b=args[1].i;if(a!=0&&((a==-1&&b==INT64_MIN)||(b==-1&&a==INT64_MIN)||(a>0&&((b>0&&a>INT64_MAX/b)||(b<0&&b<INT64_MIN/a)))||(a<0&&((b>0&&a<INT64_MIN/b)||(b<0&&a<INT64_MAX/b)))))return fw_selected_integer_overflow(name,line,column,index,vector_result);fw_set_int(out,a*b);return 1;"
            }
            ScalarKernel::IncDouble => {
                "fw_set_double(out,fw_double_arithmetic(args[0].d,1.0,FW_DOUBLE_ADD));return 1;"
            }
            ScalarKernel::DecDouble => {
                "fw_set_double(out,fw_double_arithmetic(args[0].d,1.0,FW_DOUBLE_SUB));return 1;"
            }
            ScalarKernel::NegDouble => {
                "fw_set_double(out,fw_double_from_bits(fw_double_bits(args[0].d)^UINT64_C(0x8000000000000000)));return 1;"
            }
            ScalarKernel::AbsDouble => {
                "fw_set_double(out,fw_double_from_bits(fw_double_bits(args[0].d)&UINT64_C(0x7fffffffffffffff)));return 1;"
            }
            ScalarKernel::AddDouble => {
                "fw_set_double(out,fw_double_arithmetic(args[0].d,args[1].d,FW_DOUBLE_ADD));return 1;"
            }
            ScalarKernel::SubDouble => {
                "fw_set_double(out,fw_double_arithmetic(args[0].d,args[1].d,FW_DOUBLE_SUB));return 1;"
            }
            ScalarKernel::MulDouble => {
                "fw_set_double(out,fw_double_arithmetic(args[0].d,args[1].d,FW_DOUBLE_MUL));return 1;"
            }
            ScalarKernel::EqualsBool => "fw_set_bool(out,args[0].b==args[1].b);return 1;",
            ScalarKernel::EqualsInt => "fw_set_bool(out,args[0].i==args[1].i);return 1;",
            ScalarKernel::EqualsDouble => {
                "fw_set_bool(out,fw_double_equal(args[0].d,args[1].d));return 1;"
            }
            ScalarKernel::NotEqualsBool => "fw_set_bool(out,args[0].b!=args[1].b);return 1;",
            ScalarKernel::NotEqualsInt => "fw_set_bool(out,args[0].i!=args[1].i);return 1;",
            ScalarKernel::NotEqualsDouble => {
                "fw_set_bool(out,!fw_double_equal(args[0].d,args[1].d));return 1;"
            }
            ScalarKernel::NotBool => "fw_set_bool(out,!args[0].b);return 1;",
            ScalarKernel::AndBool => "fw_set_bool(out,args[0].b&&args[1].b);return 1;",
            ScalarKernel::OrBool => "fw_set_bool(out,args[0].b||args[1].b);return 1;",
            ScalarKernel::OddInt => "fw_set_bool(out,args[0].i%2!=0);return 1;",
            ScalarKernel::EvenInt => "fw_set_bool(out,args[0].i%2==0);return 1;",
            ScalarKernel::IsPositiveInt => "fw_set_bool(out,args[0].i>0);return 1;",
            ScalarKernel::IsNegativeInt => "fw_set_bool(out,args[0].i<0);return 1;",
            ScalarKernel::IsPositiveDouble => {
                "fw_set_bool(out,!fw_double_is_nan(args[0].d)&&!fw_double_is_zero(args[0].d)&&(fw_double_bits(args[0].d)&UINT64_C(0x8000000000000000))==0U);return 1;"
            }
            ScalarKernel::IsNegativeDouble => {
                "fw_set_bool(out,!fw_double_is_nan(args[0].d)&&!fw_double_is_zero(args[0].d)&&(fw_double_bits(args[0].d)&UINT64_C(0x8000000000000000))!=0U);return 1;"
            }
            ScalarKernel::LessThanInt => "fw_set_bool(out,args[0].i<args[1].i);return 1;",
            ScalarKernel::LessThanDouble => {
                "fw_set_bool(out,fw_double_less_than(args[0].d,args[1].d));return 1;"
            }
            ScalarKernel::GreaterThanInt => "fw_set_bool(out,args[0].i>args[1].i);return 1;",
            ScalarKernel::GreaterThanDouble => {
                "fw_set_bool(out,fw_double_less_than(args[1].d,args[0].d));return 1;"
            }
            ScalarKernel::IotaInt => return Err(emission_error()),
        };
        writeln!(
            self.definitions,
            "static int fw_kernel_{implementation}(const FWV *args,FWV *out,const char *name,size_t line,size_t column,size_t index,int vector_result) {{ (void)name;(void)line;(void)column;(void)index;(void)vector_result; {body} }}"
        )
        .map_err(|_| emission_error())
    }

    fn emit_node(&mut self, index: usize, node: Node) -> Result<(), Error> {
        writeln!(
            self.definitions,
            "static int fw_ir_node_{index}(const FWV *hole, FWV *out) {{"
        )
        .map_err(|_| emission_error())?;
        match node.kind {
            NodeKind::Constant { constant } => self.emit_ir_constant(constant.0, node)?,
            NodeKind::ParameterBorrow { parameter } => {
                writeln!(
                    self.definitions,
                    "  (void)hole; return fw_borrow(&fw_parameters[{}U], out);",
                    parameter.0
                )
                .map_err(|_| emission_error())?;
            }
            NodeKind::TupleConstruct => self.emit_ir_tuple(node)?,
            NodeKind::PrefixSpreadPrepare => self.emit_ir_spread_prepare(node)?,
            NodeKind::SelectedApply {
                implementation_id,
                primitive_origin: _,
                lift,
                result_element_type: _,
                shape,
                ..
            } => self.emit_ir_selected(node, implementation_id, lift, shape)?,
            NodeKind::FanOut { branches, .. } => self.emit_ir_fan_out(node, branches)?,
        }
        self.definitions.push_str("}\n");
        Ok(())
    }

    fn emit_ir_constant(&mut self, constant: u32, node: Node) -> Result<(), Error> {
        let record = self
            .program
            .constants
            .get(to_usize(constant)?)
            .copied()
            .ok_or_else(emission_error)?;
        let origin = self.origin(node.origin.0)?;
        self.definitions.push_str("  (void)hole; ");
        match record {
            ConstantRecord::Scalar(value) => {
                self.emit_ir_scalar_assignment("out", value)?;
                self.definitions.push_str("; return 1;\n");
            }
            ConstantRecord::Vector {
                element_type,
                elements,
            } => {
                writeln!(
                    self.definitions,
                    "if (!fw_make_vector(out, {}, {}U, 0U, \"vector_literal\", {}U, {}U)) return 0;",
                    scalar_tag(element_type),
                    elements.count,
                    origin.span.begin.line,
                    origin.span.begin.column
                )
                .map_err(|_| emission_error())?;
                let values = range_slice(&self.program.constant_elements, elements)?;
                for (index, value) in values.iter().copied().enumerate() {
                    write!(self.definitions, "  ").map_err(|_| emission_error())?;
                    self.emit_ir_vector_assignment(index, value)?;
                }
                self.definitions.push_str("  return 1;\n");
            }
        }
        Ok(())
    }

    fn emit_ir_scalar_assignment(
        &mut self,
        output: &str,
        value: ScalarConstant,
    ) -> Result<(), Error> {
        match value {
            ScalarConstant::Bool(value) => {
                write!(
                    self.definitions,
                    "fw_set_bool({output}, {})",
                    i32::from(value)
                )
            }
            ScalarConstant::Int(value) => {
                write!(self.definitions, "fw_set_int({output}, {})", c_int64(value))
            }
            ScalarConstant::DoubleBits(bits) => write!(
                self.definitions,
                "fw_set_double({output}, fw_double_from_bits(UINT64_C(0x{bits:016x})))"
            ),
        }
        .map_err(|_| emission_error())
    }

    fn emit_ir_vector_assignment(
        &mut self,
        index: usize,
        value: ScalarConstant,
    ) -> Result<(), Error> {
        match value {
            ScalarConstant::Bool(value) => writeln!(
                self.definitions,
                "((unsigned char *)out->data)[{index}U] = {}U;",
                u8::from(value)
            ),
            ScalarConstant::Int(value) => writeln!(
                self.definitions,
                "((int64_t *)out->data)[{index}U] = {};",
                c_int64(value)
            ),
            ScalarConstant::DoubleBits(bits) => writeln!(
                self.definitions,
                "((double *)out->data)[{index}U] = fw_double_from_bits(UINT64_C(0x{bits:016x}));"
            ),
        }
        .map_err(|_| emission_error())
    }

    fn emit_ir_tuple(&mut self, node: Node) -> Result<(), Error> {
        let edges = range_slice(&self.program.edges, node.edges)?;
        let origin = self.origin(node.origin.0)?;
        writeln!(
            self.definitions,
            "  FWV children[{}U]; size_t initialized = 0U;",
            edges.len().max(1)
        )
        .map_err(|_| emission_error())?;
        self.definitions
            .push_str("  (void)memset(children, 0, sizeof(children));\n");
        for (position, edge) in edges.iter().copied().enumerate() {
            self.emit_edge_call(edge, &format!("&children[{position}U]"), "tuple_cleanup")?;
            writeln!(self.definitions, "  initialized = {}U;", position + 1)
                .map_err(|_| emission_error())?;
        }
        writeln!(
            self.definitions,
            "  if (!fw_make_tuple(out, {}U, \"tuple_literal\", {}U, {}U)) goto tuple_cleanup;",
            edges.len(),
            origin.span.begin.line,
            origin.span.begin.column
        )
        .map_err(|_| emission_error())?;
        for position in 0..edges.len() {
            writeln!(
                self.definitions,
                "  out->items[{position}U] = children[{position}U]; (void)memset(&children[{position}U], 0, sizeof(children[{position}U]));"
            )
            .map_err(|_| emission_error())?;
        }
        self.definitions.push_str(
            "  return 1;\ntuple_cleanup:\n  while (initialized != 0U) fw_free(&children[--initialized]);\n  return 0;\n",
        );
        Ok(())
    }

    fn emit_ir_spread_prepare(&mut self, node: Node) -> Result<(), Error> {
        let edges = range_slice(&self.program.edges, node.edges)?;
        let edge = edges.first().copied().ok_or_else(emission_error)?;
        self.emit_edge_call(edge, "out", "spread_failure")?;
        self.definitions
            .push_str("  return 1;\nspread_failure:\n  return 0;\n");
        Ok(())
    }

    fn emit_ir_selected(
        &mut self,
        node: Node,
        implementation_id: u16,
        lift: LiftMode,
        shape: crate::ShapePlan,
    ) -> Result<(), Error> {
        let _ = implementation_from_numeric(implementation_id).map_err(|_| emission_error())?;
        let edges = range_slice(&self.program.edges, node.edges)?;
        let origin = self.origin(node.origin.0)?;
        writeln!(
            self.definitions,
            "  FWV args[{}U]; FWV spread = {{0}}; size_t initialized = 0U; int has_spread = 0; int ok;",
            edges.len().max(1)
        )
        .map_err(|_| emission_error())?;
        self.definitions
            .push_str("  (void)memset(args, 0, sizeof(args));\n");
        write!(
            self.definitions,
            "  static const size_t origins[{}U][2] = {{",
            edges.len().max(1)
        )
        .map_err(|_| emission_error())?;
        for edge in edges {
            let edge_origin = self.origin(edge.origin.0)?;
            write!(
                self.definitions,
                "{{{}U,{}U}},",
                edge_origin.span.begin.line, edge_origin.span.begin.column
            )
            .map_err(|_| emission_error())?;
        }
        if edges.is_empty() {
            self.definitions.push_str("{0U,0U}");
        }
        self.definitions
            .push_str("};\n  static const int conversions[] = {");
        for edge in edges {
            write!(
                self.definitions,
                "{},",
                match edge.conversion {
                    IrConversion::Identity => 0,
                    IrConversion::PromoteIntToDouble => 1,
                }
            )
            .map_err(|_| emission_error())?;
        }
        self.definitions.push_str("0};\n");

        let mut spread_producer = None;
        for (position, edge) in edges.iter().copied().enumerate() {
            match edge.access {
                ValueAccess::WholeValue => {
                    writeln!(
                        self.definitions,
                        "  if (!fw_ir_node_{}(hole, &args[{position}U])) goto apply_cleanup;",
                        edge.producer.0
                    )
                    .map_err(|_| emission_error())?;
                }
                ValueAccess::FanOutOperandBorrow => {
                    writeln!(
                        self.definitions,
                        "  if (!fw_borrow(hole, &args[{position}U])) goto apply_cleanup;"
                    )
                    .map_err(|_| emission_error())?;
                }
                ValueAccess::TupleElement(element) => {
                    if spread_producer.is_none() {
                        writeln!(
                            self.definitions,
                            "  if (!fw_ir_node_{}(hole, &spread)) goto apply_cleanup;\n  has_spread = 1;",
                            edge.producer.0
                        )
                        .map_err(|_| emission_error())?;
                        spread_producer = Some(edge.producer.0);
                    }
                    if spread_producer != Some(edge.producer.0) {
                        return Err(emission_error());
                    }
                    writeln!(
                        self.definitions,
                        "  if (!fw_borrow(&spread.items[{element}U], &args[{position}U])) goto apply_cleanup;"
                    )
                    .map_err(|_| emission_error())?;
                }
            }
            writeln!(self.definitions, "  initialized = {}U;", position + 1)
                .map_err(|_| emission_error())?;
        }

        let shape_checks = range_slice(&self.program.shape_checks, shape.dynamic_checks)?;
        self.definitions
            .push_str("  static const size_t shape_checks[] = {");
        for global_edge in shape_checks {
            let relative = global_edge
                .checked_sub(node.edges.start)
                .ok_or_else(emission_error)?;
            write!(self.definitions, "{relative}U,").map_err(|_| emission_error())?;
        }
        self.definitions.push_str("0U};\n");
        writeln!(
            self.definitions,
            "  ok = fw_impl_{implementation_id}(args, {}U, out, {}U, {}U, origins, {}U, {}, shape_checks, {}U, conversions, {});",
            edges.len(),
            origin.span.begin.line,
            origin.span.begin.column,
            edges.len(),
            shape
                .static_anchor
                .map_or_else(|| "SIZE_MAX".to_owned(), |value| format!("{value}U")),
            shape_checks.len(),
            match lift {
                LiftMode::Scalar => 0,
                LiftMode::Vector => 1,
                LiftMode::DynamicVector => 2,
            }
        )
        .map_err(|_| emission_error())?;
        self.definitions.push_str(
            "  while (initialized != 0U) fw_free(&args[--initialized]);\n  if (has_spread) fw_free(&spread);\n  return ok;\napply_cleanup:\n  while (initialized != 0U) fw_free(&args[--initialized]);\n  if (has_spread) fw_free(&spread);\n  return 0;\n",
        );
        Ok(())
    }

    fn emit_ir_fan_out(&mut self, node: Node, branches: IndexRange) -> Result<(), Error> {
        let edges = range_slice(&self.program.edges, node.edges)?;
        let operand = edges.first().copied().ok_or_else(emission_error)?;
        let branch_records = range_slice(&self.program.branches, branches)?;
        let origin = self.origin(node.origin.0)?;
        self.definitions
            .push_str("  FWV operand = {0}; size_t initialized = 0U;\n");
        match operand.access {
            ValueAccess::WholeValue => writeln!(
                self.definitions,
                "  if (!fw_ir_node_{}(hole, &operand)) return 0;",
                operand.producer.0
            ),
            ValueAccess::FanOutOperandBorrow => {
                writeln!(
                    self.definitions,
                    "  if (!fw_borrow(hole, &operand)) return 0;"
                )
            }
            ValueAccess::TupleElement(_) => return Err(emission_error()),
        }
        .map_err(|_| emission_error())?;
        writeln!(
            self.definitions,
            "  if (!fw_make_tuple(out, {}U, \"fanout\", {}U, {}U)) {{ fw_free(&operand); return 0; }}",
            branch_records.len(),
            origin.span.begin.line,
            origin.span.begin.column
        )
        .map_err(|_| emission_error())?;
        for (position, branch) in branch_records.iter().enumerate() {
            writeln!(
                self.definitions,
                "  if (!fw_ir_node_{}(&operand, &out->items[{position}U])) goto fanout_cleanup;\n  initialized = {}U;",
                branch.root.0,
                position + 1
            )
            .map_err(|_| emission_error())?;
        }
        self.definitions
            .push_str("  (void)initialized; fw_free(&operand); return 1;\n");
        if !branch_records.is_empty() {
            self.definitions.push_str(
                "fanout_cleanup:\n  (void)initialized; fw_free(out); fw_free(&operand); return 0;\n",
            );
        }
        Ok(())
    }

    fn emit_edge_call(
        &mut self,
        edge: crate::Edge,
        output: &str,
        failure: &str,
    ) -> Result<(), Error> {
        match edge.access {
            ValueAccess::WholeValue => writeln!(
                self.definitions,
                "  if (!fw_ir_node_{}(hole, {output})) goto {failure};",
                edge.producer.0
            ),
            ValueAccess::FanOutOperandBorrow => writeln!(
                self.definitions,
                "  if (!fw_borrow(hole, {output})) goto {failure};"
            ),
            ValueAccess::TupleElement(_) => return Err(emission_error()),
        }
        .map_err(|_| emission_error())
    }

    fn origin(&self, index: u32) -> Result<Origin, Error> {
        self.program
            .origins
            .get(to_usize(index)?)
            .copied()
            .ok_or_else(emission_error)
    }
}

fn to_usize(value: u32) -> Result<usize, Error> {
    usize::try_from(value).map_err(|_| emission_error())
}

fn range_slice<T>(values: &[T], range: IndexRange) -> Result<&[T], Error> {
    let start = to_usize(range.start)?;
    let end = range
        .checked_end()
        .ok_or_else(emission_error)
        .and_then(to_usize)?;
    values.get(start..end).ok_or_else(emission_error)
}

fn parameter_runtime() -> Result<String, Error> {
    let mut runtime = String::new();
    runtime
        .try_reserve_exact(PARAMETER_RUNTIME.len())
        .map_err(|_| emission_error())?;
    runtime.push_str(PARAMETER_RUNTIME);
    Ok(runtime)
}

pub fn emit_c_source(source: &str) -> Result<CEmissionResult, Error> {
    emit_c_source_with_configuration(source, EvaluationConfiguration::default())
}

pub fn emit_c_source_with_configuration(
    source: &str,
    configuration: EvaluationConfiguration,
) -> Result<CEmissionResult, Error> {
    let resources = crate::resources::ResourceContext::new(
        configuration.profile,
        configuration.limits,
        configuration.allocation_failure,
    )?;
    let parsed = parse(source)?;
    validate_parameter_declarations(&parsed)?;
    resolve_names(&parsed)?;
    if program_contains_tuple(&parsed) {
        let first = first_tuple_location(&parsed).unwrap_or_else(crate::SourceLocation::start);
        resources.require_tuple_profile(first)?;
    }
    let program = compile_parsed_source(source, &parsed)
        .map_err(crate::lowering::CompileError::into_evaluation_error)?;
    emit_verified_c_program(&program, configuration)
}

const fn scalar_tag(scalar: ScalarType) -> i32 {
    match scalar {
        ScalarType::Bool => 0,
        ScalarType::Int => 1,
        ScalarType::Double => 2,
    }
}

fn c_int64(value: i64) -> String {
    if value == i64::MIN {
        "(-INT64_C(9223372036854775807) - INT64_C(1))".to_owned()
    } else {
        format!("INT64_C({value})")
    }
}

fn c_string(text: &str) -> String {
    let mut output = String::from("\"");
    for byte in text.bytes() {
        match byte {
            b'\\' => output.push_str("\\\\"),
            b'"' => output.push_str("\\\""),
            b'\n' => output.push_str("\\n"),
            b'\r' => output.push_str("\\r"),
            b'\t' => output.push_str("\\t"),
            0x20..=0x7e => output.push(char::from(byte)),
            _ => {
                let _ = write!(output, "\\{byte:03o}");
            }
        }
    }
    output.push('"');
    output
}

fn emission_error() -> Error {
    Error::new(
        ErrorKind::FormattingError,
        crate::SourceLocation::start(),
        "unable to construct generated C source",
    )
}

const PARAMETER_RUNTIME: &str = r#"/* Generated by Faraweave 0.1.0. Strict C11. */
#include <errno.h>
#include <inttypes.h>
#include <locale.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif
#if defined(__x86_64__) || defined(_M_X64)
#include <xmmintrin.h>
#endif

typedef struct FWV FWV;
struct FWV {
  int kind, type, owns, b;
  size_t len, charge, allocation_ordinal, cursor;
  int64_t i;
  double d;
  void *data;
  FWV *items;
  FWV *parent;
};
typedef int (*FWExpr)(const FWV *, FWV *);
extern FWV fw_parameters[];
extern const int fw_parameter_types[];
extern const char *const fw_parameter_names[];
extern const size_t fw_parameter_spans[][6];
extern const size_t fw_required;
extern const int fw_profile;
extern const int fw_has_vector_limit;
extern const size_t fw_vector_limit;
extern const int fw_has_tuple_limit;
extern const size_t fw_tuple_limit;
extern const int fw_has_live_limit;
extern const size_t fw_live_limit;
extern const int fw_has_work_limit;
extern const size_t fw_work_limit;
extern const int fw_has_failure_ordinal;
extern const size_t fw_failure_ordinal;
static const char *fw_error_kind = NULL;
static const char *fw_error_message = NULL;
static size_t fw_error_line = 1U, fw_error_column = 1U;
static char fw_error_storage[256];
static size_t fw_live_bytes=0U,fw_peak_live_bytes=0U,fw_work_units=0U;
static size_t fw_allocation_attempts=0U;

static int fw_fail(const char *kind, const char *message, size_t line, size_t column) {
  if (fw_error_kind == NULL) {
    fw_error_kind = kind; fw_error_message = message;
    fw_error_line = line == 0U ? 1U : line;
    fw_error_column = column == 0U ? 1U : column;
  }
  return 0;
}
static int fw_fail_primitive(const char *kind, const char *name, const char *reason,
                             size_t line, size_t column) {
  (void)snprintf(fw_error_storage, sizeof(fw_error_storage), "%s failed: %s", name, reason);
  return fw_fail(kind, fw_error_storage, line, column);
}
static int fw_fail_shape(const char *name, size_t argument, size_t expected,
                         size_t actual, size_t line, size_t column) {
  (void)snprintf(fw_error_storage, sizeof(fw_error_storage),
                 "%s argument %zu expected shape [%zu], got [%zu]",
                 name, argument, expected, actual);
  return fw_fail("ShapeMismatch", fw_error_storage, line, column);
}
static double fw_double_from_bits(uint64_t bits) {
  double value; (void)memcpy(&value, &bits, sizeof(value)); return value;
}
static uint64_t fw_double_bits(double value) {
  uint64_t bits; (void)memcpy(&bits, &value, sizeof(bits)); return bits;
}
static int fw_double_is_nan(double value) {
  uint64_t bits=fw_double_bits(value);
  return (bits&UINT64_C(0x7ff0000000000000))==UINT64_C(0x7ff0000000000000) &&
         (bits&UINT64_C(0x000fffffffffffff))!=UINT64_C(0);
}
static int fw_double_is_infinity(double value) {
  return (fw_double_bits(value)&UINT64_C(0x7fffffffffffffff))==UINT64_C(0x7ff0000000000000);
}
static int fw_double_is_zero(double value) {
  return (fw_double_bits(value)&UINT64_C(0x7fffffffffffffff))==UINT64_C(0);
}
static uint64_t fw_double_order_key(double value) {
  uint64_t bits=fw_double_bits(value);
  return (bits&UINT64_C(0x8000000000000000))!=UINT64_C(0)
      ?~bits:bits|UINT64_C(0x8000000000000000);
}
static int fw_double_equal(double left,double right) {
  if(fw_double_is_nan(left)||fw_double_is_nan(right))return 0;
  if(fw_double_is_zero(left)&&fw_double_is_zero(right))return 1;
  return fw_double_bits(left)==fw_double_bits(right);
}
static int fw_double_less_than(double left,double right) {
  if(fw_double_is_nan(left)||fw_double_is_nan(right)||
     (fw_double_is_zero(left)&&fw_double_is_zero(right)))return 0;
  return fw_double_order_key(left)<fw_double_order_key(right);
}
static double fw_int_to_double(int64_t value) {
  int negative=value<INT64_C(0); uint64_t magnitude,scan,significand=UINT64_C(0);
  uint64_t fraction,exponent; unsigned int most_significant=0U;
  if(value==INT64_C(0))return 0.0;
  magnitude=negative?(uint64_t)(-(value+INT64_C(1)))+UINT64_C(1):(uint64_t)value;
  scan=magnitude;while(scan>UINT64_C(1)){scan>>=1U;++most_significant;}
  if(most_significant<=52U)significand=magnitude<<(52U-most_significant);
  else {
    unsigned int shift=most_significant-52U;
    uint64_t mask=(UINT64_C(1)<<shift)-UINT64_C(1),remainder=magnitude&mask;
    uint64_t halfway=UINT64_C(1)<<(shift-1U);significand=magnitude>>shift;
    if(remainder>halfway||(remainder==halfway&&(significand&UINT64_C(1))!=0U)){
      ++significand;if(significand==(UINT64_C(1)<<53U)){significand>>=1U;++most_significant;}
    }
  }
  exponent=(uint64_t)(most_significant+1023U)<<52U;
  fraction=significand&UINT64_C(0x000fffffffffffff);
  return fw_double_from_bits((negative?UINT64_C(0x8000000000000000):UINT64_C(0))|
                             exponent|fraction);
}
#if defined(__x86_64__) || defined(_M_X64)
typedef struct FWStrictEnvironment {
  unsigned int control;
#if !defined(_MSC_VER)
  unsigned char x87[28];
#endif
} FWStrictEnvironment;
static void fw_begin_strict_environment(FWStrictEnvironment *environment) {
  environment->control=_mm_getcsr();
#if !defined(_MSC_VER)
  { uint16_t control=UINT16_C(0);
    __asm__ volatile("fnstenv %0":"=m"(environment->x87));
    (void)memcpy(&control,environment->x87,sizeof(control));
    control=(uint16_t)((control|UINT16_C(0x003f))&~UINT16_C(0x0c00));
    __asm__ volatile("fldcw %0"::"m"(control));
  }
#endif
  _mm_setcsr((environment->control|0x1f80U)&~(0x003fU|0x0040U|0x6000U|0x8000U));
}
static void fw_restore_strict_environment(const FWStrictEnvironment *environment) {
#if !defined(_MSC_VER)
  __asm__ volatile("fldenv %0"::"m"(environment->x87));
#endif
  _mm_setcsr(environment->control);
}
#elif defined(__aarch64__)
typedef struct FWStrictEnvironment { uint64_t control,status; } FWStrictEnvironment;
static void fw_begin_strict_environment(FWStrictEnvironment *environment) {
  uint64_t strict_control,clear_status=UINT64_C(0);
  __asm__ volatile("mrs %0, fpcr":"=r"(environment->control));
  __asm__ volatile("mrs %0, fpsr":"=r"(environment->status));
  strict_control=environment->control&
      ~(UINT64_C(0x00009f00)|UINT64_C(0x00c00000)|UINT64_C(0x03000000));
  __asm__ volatile("msr fpcr, %0\n\tisb"::"r"(strict_control):"memory");
  __asm__ volatile("msr fpsr, %0"::"r"(clear_status):"memory");
}
static void fw_restore_strict_environment(const FWStrictEnvironment *environment) {
  __asm__ volatile("msr fpcr, %0\n\tisb"::"r"(environment->control):"memory");
  __asm__ volatile("msr fpsr, %0"::"r"(environment->status):"memory");
}
#else
#error "Faraweave requires an x86-64 or AArch64 floating-point environment"
#endif
enum { FW_DOUBLE_ADD=0,FW_DOUBLE_SUB=1,FW_DOUBLE_MUL=2 };
static double fw_double_arithmetic(double left,double right,int operation) {
  uint64_t left_bits=fw_double_bits(left),right_bits=fw_double_bits(right);
  int signs_differ=((left_bits^right_bits)&UINT64_C(0x8000000000000000))!=0U;
  if(fw_double_is_nan(left)||fw_double_is_nan(right)||
     (fw_double_is_infinity(left)&&fw_double_is_infinity(right)&&
      ((operation==FW_DOUBLE_ADD&&signs_differ)||
       (operation==FW_DOUBLE_SUB&&!signs_differ)))||
     (operation==FW_DOUBLE_MUL&&
      ((fw_double_is_infinity(left)&&fw_double_is_zero(right))||
       (fw_double_is_zero(left)&&fw_double_is_infinity(right)))))
    return fw_double_from_bits(UINT64_C(0x7ff8000000000000));
  { FWStrictEnvironment environment;volatile double a=left,b=right,result=0.0;
    fw_begin_strict_environment(&environment);
    if(operation==FW_DOUBLE_ADD)result=a+b;
    else if(operation==FW_DOUBLE_SUB)result=a-b;
    else result=a*b;
    fw_restore_strict_environment(&environment);
    return fw_double_is_nan(result)
        ?fw_double_from_bits(UINT64_C(0x7ff8000000000000)):result;
  }
}
static void fw_set_bool(FWV *out, int value) {
  (void)memset(out, 0, sizeof(*out)); out->b = value != 0;
}
static void fw_set_int(FWV *out, int64_t value) {
  (void)memset(out, 0, sizeof(*out)); out->type = 1; out->i = value;
}
static void fw_set_double(FWV *out, double value) {
  (void)memset(out, 0, sizeof(*out)); out->type = 2;
  out->d = fw_double_is_nan(value)
      ? fw_double_from_bits(UINT64_C(0x7ff8000000000000)) : value;
}
static int fw_borrow(const FWV *value, FWV *out) {
  if (value == NULL) return 0;
  *out = *value; out->owns = 0; return 1;
}
static size_t fw_width(int type) { return type == 0 ? 1U : 8U; }
static int fw_fail_resource(const char *producer,const char *reason,
                            size_t line,size_t column) {
  (void)snprintf(fw_error_storage,sizeof(fw_error_storage),
                 "%s resource request failed: %s",producer,reason);
  return fw_fail("ResourceError",fw_error_storage,line,column);
}
static int fw_admit(size_t bytes,int tuple,size_t work,const char *producer,
                    size_t line,size_t column,size_t *ordinal) {
  size_t live_after,work_after;
  if(bytes>SIZE_MAX-fw_live_bytes||work>SIZE_MAX-fw_work_units)
    return fw_fail_resource(producer,"size_overflow",line,column);
  live_after=fw_live_bytes+bytes;work_after=fw_work_units+work;
  if((tuple?fw_has_tuple_limit:fw_has_vector_limit)&&
     bytes>(tuple?fw_tuple_limit:fw_vector_limit))
    return fw_fail_resource(producer,"profile_limit",line,column);
  if(fw_has_live_limit&&live_after>fw_live_limit)
    return fw_fail_resource(producer,"profile_limit",line,column);
  if(fw_has_work_limit&&work_after>fw_work_limit)
    return fw_fail_resource(producer,"profile_limit",line,column);
  *ordinal=SIZE_MAX;
  if(bytes!=0U){
    if(fw_allocation_attempts==SIZE_MAX)
      return fw_fail_resource(producer,"size_overflow",line,column);
    *ordinal=fw_allocation_attempts;++fw_allocation_attempts;
    if(fw_has_failure_ordinal&&*ordinal==fw_failure_ordinal)
      return fw_fail_resource(producer,"allocation_unavailable",line,column);
  }
  fw_live_bytes=live_after;fw_work_units=work_after;
  if(live_after>fw_peak_live_bytes)fw_peak_live_bytes=live_after;
  return 1;
}
static void fw_refund(size_t bytes) {
  fw_live_bytes=bytes<=fw_live_bytes?fw_live_bytes-bytes:0U;
}
static int fw_charge_work(size_t amount,const char *producer,size_t line,size_t column) {
  size_t after;if(amount>SIZE_MAX-fw_work_units)
    return fw_fail_resource(producer,"size_overflow",line,column);
  after=fw_work_units+amount;
  if(fw_has_work_limit&&after>fw_work_limit)
    return fw_fail_resource(producer,"profile_limit",line,column);
  fw_work_units=after;return 1;
}
static int fw_make_vector(FWV *out, int type, size_t length,size_t work,const char *producer,
                          size_t line,size_t column) {
  size_t bytes;
  (void)memset(out, 0, sizeof(*out)); out->kind = 1; out->type = type; out->len = length; out->owns = 1;
  if (length > SIZE_MAX / fw_width(type)) return fw_fail_resource(producer,"size_overflow",line,column);
  bytes=length*fw_width(type);
  if(!fw_admit(bytes,0,work,producer,line,column,&out->allocation_ordinal))return 0;
  if (length == 0U) return 1;
  out->data = calloc(length, fw_width(type));
  if(out->data==NULL){fw_refund(bytes);return fw_fail_resource(producer,"allocation_unavailable",line,column);}
  out->charge=bytes;return 1;
}
static int fw_make_tuple(FWV *out, size_t length,const char *producer,
                         size_t line,size_t column) {
  size_t bytes;
  (void)memset(out, 0, sizeof(*out)); out->kind = 2; out->len = length; out->owns = 1;
  if(fw_profile<2)return fw_fail("ProfileError",
      fw_profile==0?"trusted-local-v1 does not support value kind Tuple":
                    "bounded-v1 does not support value kind Tuple",line,column);
  if (length == 0U) return 1;
  if (length > SIZE_MAX / 16U || length > SIZE_MAX / sizeof(FWV))
    return fw_fail_resource(producer,"size_overflow",line,column);
  bytes=length*16U;
  if(!fw_admit(bytes,1,0U,producer,line,column,&out->allocation_ordinal))return 0;
  out->items = (FWV *)calloc(length, sizeof(FWV));
  if(out->items==NULL){fw_refund(bytes);return fw_fail_resource(producer,"allocation_unavailable",line,column);}
  out->charge=bytes;return 1;
}
static void fw_free(FWV *value) {
  FWV *current=value,*parent;void *storage;size_t charge;
  if(current!=NULL)current->parent=NULL;
  while(current!=NULL){
    if(!current->owns){
      parent=current->parent;(void)memset(current,0,sizeof(*current));current=parent;continue;
    }
    if(current->kind==2&&current->cursor<current->len){
      FWV *child=&current->items[current->len-1U-current->cursor++];
      child->parent=current;current=child;continue;
    }
    parent=current->parent;charge=current->charge;
    storage=current->kind==2?(void *)current->items:current->data;
    free(storage);(void)memset(current,0,sizeof(*current));fw_refund(charge);
    current=parent;
  }
}
static FWV fw_scalar_at(const FWV *value, size_t index) {
  FWV result = {0};
  result.type = value->type;
  if (value->kind == 0) return *value;
  if (value->type == 0) result.b = ((const unsigned char *)value->data)[index] != 0U;
  else if (value->type == 1) result.i = ((const int64_t *)value->data)[index];
  else result.d = ((const double *)value->data)[index];
  return result;
}
static int fw_put_scalar(FWV *value, size_t index, FWV scalar) {
  if (value->kind == 0) { *value = scalar; return 1; }
  if (value->type == 0) ((unsigned char *)value->data)[index] = (unsigned char)scalar.b;
  else if (value->type == 1) ((int64_t *)value->data)[index] = scalar.i;
  else ((double *)value->data)[index] = scalar.d;
  return 1;
}
static double fw_as_double(FWV value) {
  return value.type == 2 ? value.d : fw_int_to_double(value.i);
}
typedef int (*FWSelectedKernel)(const FWV *,FWV *,const char *,size_t,size_t,size_t,int);
static int fw_selected_integer_overflow(const char *name,size_t line,size_t column,
                                        size_t index,int vector_result) {
  if(vector_result){
    (void)snprintf(fw_error_storage,sizeof(fw_error_storage),
                   "%s failed: integer_overflow at result index %zu",name,index);
    return fw_fail("DomainError",fw_error_storage,line,column);
  }
  return fw_fail_primitive("DomainError",name,"integer_overflow",line,column);
}
static FWV fw_scalar_at_selected(const FWV *value,size_t index,int conversion) {
  FWV result=fw_scalar_at(value,index);
  if(conversion!=0){int64_t integer=result.i;fw_set_double(&result,fw_int_to_double(integer));}
  return result;
}
static int fw_apply_selected(FWSelectedKernel kernel,const char *name,int result_type,
                             const FWV *args,size_t count,FWV *out,
                             size_t line,size_t column,const size_t (*origins)[2],
                             size_t origin_count,size_t static_anchor,
                             const size_t *shape_checks,size_t shape_count,
                             const int *conversions,int lift) {
  size_t i,length=1U,anchor=static_anchor;
  if(lift!=0){
    if(anchor==SIZE_MAX){
      if(shape_count==0U)return fw_fail("ValueError","selected vector plan has no anchor",line,column);
      anchor=shape_checks[0];
    }
    length=args[anchor].len;
    for(i=0U;i<shape_count;++i){
      size_t position=shape_checks[i];
      if(position!=anchor&&args[position].len!=length){
        size_t origin_line=position<origin_count?origins[position][0]:line;
        size_t origin_column=position<origin_count?origins[position][1]:column;
        return fw_fail_shape(name,position+1U,length,args[position].len,
                             origin_line,origin_column);
      }
    }
    if(!fw_make_vector(out,result_type,length,length,name,line,column))return 0;
  }else{
    (void)memset(out,0,sizeof(*out));out->type=result_type;
    if(!fw_charge_work(1U,name,line,column))return 0;
  }
  for(i=0U;i<length;++i){
    FWV scalar_args[2]={{0}};FWV scalar_out={0};size_t j;
    for(j=0U;j<count;++j)
      scalar_args[j]=fw_scalar_at_selected(&args[j],i,conversions[j]);
    if(!kernel(scalar_args,&scalar_out,name,line,column,i,lift!=0)){
      fw_free(out);return 0;
    }
    (void)fw_put_scalar(out,i,scalar_out);
  }
  return 1;
}
static int fw_apply_selected_iota(const char *name,const FWV *args,FWV *out,
                                  size_t line,size_t column) {
  int64_t bound=args[0].i;
  size_t i,length=bound>0?(size_t)bound:0U;
  if(!fw_make_vector(out,1,length,length,name,line,column))return 0;
  for(i=0U;i<length;++i)((int64_t *)out->data)[i]=(int64_t)i+1;
  return 1;
}
typedef struct { char *data; size_t size, capacity; } FWBuffer;
static int fw_append(FWBuffer *buffer,const char *text) {
  size_t n=strlen(text),needed; char *grown;
  if(n>SIZE_MAX-buffer->size-1U) return 0;
  needed=buffer->size+n+1U;
  if(needed>buffer->capacity) { size_t next=buffer->capacity?buffer->capacity:128U;
    while(next<needed){if(next>SIZE_MAX/2U)return 0;next*=2U;}
    grown=(char *)realloc(buffer->data,next);if(grown==NULL)return 0;buffer->data=grown;buffer->capacity=next;}
  (void)memcpy(buffer->data+buffer->size,text,n+1U);buffer->size+=n;return 1;
}
static void fw_normalize_exponent(const char *input,char *output,size_t capacity) {
  const char *exponent=strchr(input,'e'),*upper=strchr(input,'E'),*digits;
  size_t used=0U;int negative=0;
  if(exponent==NULL)exponent=upper;
  if(exponent==NULL){(void)snprintf(output,capacity,"%s",input);return;}
  while(input!=exponent&&used+1U<capacity)output[used++]=*input++;
  if(used+1U<capacity)output[used++]='e';
  digits=exponent+1;if(*digits=='+'||*digits=='-'){negative=*digits=='-';++digits;}
  while(digits[0]=='0'&&digits[1]!='\0')++digits;
  if(negative&&used+1U<capacity)output[used++]='-';
  while(*digits!='\0'&&used+1U<capacity)output[used++]=*digits++;
  output[used]='\0';
}
static int fw_format_double(char *output,size_t capacity,double value) {
  uint64_t bits=fw_double_bits(value);double magnitude;
  char candidate[64],normalized[64];int precision,matched=0,ok=0;
  FWStrictEnvironment environment;
  if(fw_double_is_nan(value)){(void)snprintf(output,capacity,"nan");return 1;}
  if(fw_double_is_infinity(value)){
    (void)snprintf(output,capacity,"%s",(bits&UINT64_C(0x8000000000000000))?"-inf":"inf");
    return 1;
  }
  if(bits==UINT64_C(0)){(void)snprintf(output,capacity,"0.0");return 1;}
  if(bits==UINT64_C(0x8000000000000000)){(void)snprintf(output,capacity,"-0.0");return 1;}
  magnitude=fw_double_from_bits(bits&UINT64_C(0x7fffffffffffffff));candidate[0]='\0';
  fw_begin_strict_environment(&environment);
  if(magnitude>=1000000.0||magnitude<0.0001){
    for(precision=0;precision<=16;++precision){
      char *end=NULL;double parsed;
      if(snprintf(candidate,sizeof(candidate),"%.*e",precision,value)<0)goto done;
      parsed=strtod(candidate,&end);
      if(end!=NULL&&*end=='\0'&&fw_double_bits(parsed)==bits){matched=1;break;}
    }
  }else{
    for(precision=0;precision<=20;++precision){
      char *end=NULL;double parsed;
      if(snprintf(candidate,sizeof(candidate),"%.*f",precision,value)<0)goto done;
      parsed=strtod(candidate,&end);
      if(end!=NULL&&*end=='\0'&&fw_double_bits(parsed)==bits){matched=1;break;}
    }
  }
  if(!matched)goto done;
  fw_normalize_exponent(candidate,normalized,sizeof(normalized));
  if(strchr(normalized,'.')==NULL&&strchr(normalized,'e')==NULL){
    if(snprintf(output,capacity,"%s.0",normalized)<0)goto done;
  }else if(snprintf(output,capacity,"%s",normalized)<0)goto done;
  ok=1;
done:
  fw_restore_strict_environment(&environment);return ok;
}
typedef struct { const FWV *value; size_t index; } FWFormatFrame;
static int fw_format(FWBuffer *buffer,const FWV *value) {
  FWFormatFrame *stack=NULL,*grown;size_t depth=0U,capacity=0U,i;char text[128];int ok=0;
  if(capacity==depth){capacity=16U;stack=(FWFormatFrame *)malloc(capacity*sizeof(*stack));if(stack==NULL)return 0;}
  stack[depth++]=(FWFormatFrame){value,0U};
  while(depth!=0U){
    FWFormatFrame *frame=&stack[depth-1U];const FWV *current=frame->value;
    if(current->kind==2){
      if(frame->index==0U&&!fw_append(buffer,"["))goto done;
      if(frame->index<current->len){
        const FWV *child=&current->items[frame->index];
        if(frame->index!=0U&&!fw_append(buffer," "))goto done;
        ++frame->index;
        if(depth==capacity){
          size_t next;if(capacity>SIZE_MAX/2U)goto done;next=capacity*2U;
          if(next>SIZE_MAX/sizeof(*stack))goto done;
          grown=(FWFormatFrame *)realloc(stack,next*sizeof(*stack));if(grown==NULL)goto done;
          stack=grown;capacity=next;
        }
        stack[depth++]=(FWFormatFrame){child,0U};continue;
      }
      if(!fw_append(buffer,"]"))goto done;
      --depth;continue;
    }
    if(current->kind==1){
      if(!fw_append(buffer,"("))goto done;
      for(i=0U;i<current->len;++i){
        FWV scalar=fw_scalar_at(current,i);
        if(i!=0U&&!fw_append(buffer," "))goto done;
        if(scalar.type==0){if(!fw_append(buffer,scalar.b?"true":"false"))goto done;}
        else if(scalar.type==1){if(snprintf(text,sizeof(text),"%" PRId64,scalar.i)<=0||!fw_append(buffer,text))goto done;}
        else if(!fw_format_double(text,sizeof(text),scalar.d)||!fw_append(buffer,text))goto done;
      }
      if(!fw_append(buffer,")"))goto done;
      --depth;continue;
    }
    if(current->type==0){if(!fw_append(buffer,current->b?"true":"false"))goto done;}
    else if(current->type==1){if(snprintf(text,sizeof(text),"%" PRId64,current->i)<=0||!fw_append(buffer,text))goto done;}
    else if(!fw_format_double(text,sizeof(text),current->d)||!fw_append(buffer,text))goto done;
    --depth;
  }
  ok=1;
done:
  free(stack);return ok;
}
static int fw_ascii_digits(const char *text) {
  if(*text=='\0')return 0;
  while(*text!='\0'){if(*text<'0'||*text>'9')return 0;++text;}return 1;
}
static int fw_finite_double_grammar(const char *text) {
  const char *cursor=text,*integer;size_t integer_count=0U;int fraction=0,exponent=0;
  if(*cursor=='-')++cursor;else if(*cursor=='+')return 0;
  integer=cursor;while(*cursor>='0'&&*cursor<='9'){++integer_count;++cursor;}
  if(integer_count==0U||(integer[0]=='0'&&integer_count!=1U))return 0;
  if(*cursor=='.'){
    ++cursor;while(*cursor>='0'&&*cursor<='9'){fraction=1;++cursor;}
    if(!fraction)return 0;
  }
  if(*cursor=='e'||*cursor=='E'){
    ++cursor;if(*cursor=='+'||*cursor=='-')++cursor;
    while(*cursor>='0'&&*cursor<='9'){exponent=1;++cursor;}
    if(!exponent)return 0;
  }
  return *cursor=='\0'&&(fraction||exponent);
}
enum { FW_DECODE_INVALID=0,FW_DECODE_OK=1,FW_DECODE_RANGE=2 };
static int fw_decode(const char *text,int type,FWV *out) {
  char *end=NULL;
  if(type==0){if(strcmp(text,"true")==0){fw_set_bool(out,1);return FW_DECODE_OK;}if(strcmp(text,"false")==0){fw_set_bool(out,0);return FW_DECODE_OK;}return FW_DECODE_INVALID;}
  if(type==1){
    const char *digits=*text=='-'?text+1:text;
    if(!fw_ascii_digits(digits)||*text=='+'||(digits[0]=='0'&&digits[1]!='\0')||
       (*text=='-'&&digits[0]=='0'&&digits[1]=='\0'))return FW_DECODE_INVALID;
    errno=0;out->i=strtoimax(text,&end,10);out->type=1;
    return errno==ERANGE||end==text||*end!='\0'?FW_DECODE_RANGE:FW_DECODE_OK;
  }
  if(strcmp(text,"inf")==0){fw_set_double(out,fw_double_from_bits(UINT64_C(0x7ff0000000000000)));return 1;}
  if(strcmp(text,"-inf")==0){fw_set_double(out,fw_double_from_bits(UINT64_C(0xfff0000000000000)));return 1;}
  if(strcmp(text,"nan")==0){fw_set_double(out,fw_double_from_bits(UINT64_C(0x7ff8000000000000)));return 1;}
  if(!fw_finite_double_grammar(text))return FW_DECODE_INVALID;
  { FWStrictEnvironment environment;errno=0;
    fw_begin_strict_environment(&environment);
    out->d=strtod(text,&end);
    fw_restore_strict_environment(&environment);out->type=2;
    return end==text||*end!='\0'||fw_double_is_infinity(out->d)
        ?FW_DECODE_RANGE:FW_DECODE_OK;
  }
}
static const char *fw_type_name(int type) {
  return type==0?"Bool":type==1?"Int":"Double";
}
static int fw_report_argument(const char *reason,size_t supplied,size_t position) {
  if(position<=fw_required){
    const size_t *span=fw_parameter_spans[position-1U];
    return fprintf(stderr,
      "faraweave_argument_error reason=%s required_count=%zu supplied_count=%zu position=%zu parameter_name=%s expected_type=%s declaration_span=%zu:%zu:%zu-%zu:%zu:%zu actual_container=- actual_type=- invalid_value_invariant=-\n",
      reason,fw_required,supplied,position,fw_parameter_names[position-1U],
      fw_type_name(fw_parameter_types[position-1U]),span[0],span[1],span[2],
      span[3],span[4],span[5])<0?0:1;
  }
  return fprintf(stderr,
    "faraweave_argument_error reason=%s required_count=%zu supplied_count=%zu position=%zu parameter_name=- expected_type=- declaration_span=- actual_container=- actual_type=- invalid_value_invariant=-\n",
    reason,fw_required,supplied,position)<0?0:1;
}
static int fw_main(int argc,char **argv,size_t root_count,const FWExpr *roots) {
  size_t supplied=argc>0?(size_t)argc-1U:0U,i,initialized=0U;FWV *values=NULL;FWBuffer output={0};int decoded;
  (void)fw_apply_selected;
  (void)fw_apply_selected_iota;
  (void)fw_selected_integer_overflow;
  (void)fw_as_double;
  (void)fw_borrow;
  (void)fw_double_arithmetic;
  (void)fw_double_equal;
  (void)fw_double_less_than;
  (void)fw_set_int;
  (void)fw_make_tuple;
#ifdef _WIN32
  if (_setmode(_fileno(stdout), _O_BINARY) == -1 || _setmode(_fileno(stderr), _O_BINARY) == -1) return 1;
#endif
  if(setvbuf(stdout,NULL,_IONBF,0)!=0)return 1;
  if(setlocale(LC_NUMERIC,"C")==NULL)return 1;
  if(supplied!=fw_required){size_t pos=supplied<fw_required?supplied+1U:fw_required+1U;(void)fw_report_argument(supplied<fw_required?"missing":"extra",supplied,pos);return 1;}
  for(i=0U;i<fw_required;++i){decoded=fw_decode(argv[i+1U],fw_parameter_types[i],&fw_parameters[i]);if(decoded!=FW_DECODE_OK){(void)fw_report_argument(decoded==FW_DECODE_RANGE?"out_of_range":"invalid_literal",supplied,i+1U);return 1;}}
  values=(FWV *)calloc(root_count?root_count:1U,sizeof(FWV));if(values==NULL)return 1;
  for(i=0U;i<root_count;++i){if(!roots[i](NULL,&values[i]))goto failure;initialized=i+1U;}
  for(i=0U;i<root_count;++i)if(!fw_format(&output,&values[i])||!fw_append(&output,"\n"))goto failure;
  while(initialized)fw_free(&values[--initialized]);
  free(values);
  if(output.size){size_t accepted=fwrite(output.data,1U,output.size,stdout);
    if(accepted!=output.size){(void)fprintf(stderr,
      "faraweave_output_error reason=write_failed pending_byte_count=%zu accepted_byte_count=%zu output_position=%zu\n",
      output.size,accepted,accepted);free(output.data);return 1;}}
  {size_t published=output.size;free(output.data);
    if(fflush(stdout)!=0){(void)fprintf(stderr,
      "faraweave_output_error reason=flush_failed pending_byte_count=%zu accepted_byte_count=%zu output_position=%zu\n",
      published,published,published);return 1;}}return 0;
failure:
  while(initialized)fw_free(&values[--initialized]);
  free(values);free(output.data);
  (void)fprintf(stderr,"<generated>:%zu:%zu: %s: %s\n",fw_error_line,fw_error_column,fw_error_kind?fw_error_kind:"FormattingError",fw_error_message?fw_error_message:"unable to format result");return 1;
}
"#;

#[cfg(test)]
mod ir_tests {
    use super::*;
    use crate::lowering::compile_source_with_name;

    fn must<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("fixture failed: {error:?}"),
        }
    }

    fn emit(source: &str) -> String {
        emit_with_configuration(
            source,
            EvaluationConfiguration {
                profile: crate::ExecutionProfile::TrustedLocalV2,
                ..EvaluationConfiguration::default()
            },
        )
    }

    fn emit_with_configuration(source: &str, configuration: EvaluationConfiguration) -> String {
        let program = match compile_source_with_name(source, "<source>") {
            Ok(program) => program,
            Err(error) => panic!("test source did not lower: {error}"),
        };
        match emit_verified_c_program(&program, configuration) {
            Ok(emission) => emission.source,
            Err(error) => panic!("verified program did not emit: {error}"),
        }
    }

    #[test]
    fn verified_generator_uses_ir_identities_conversions_shapes_and_provenance() {
        let source = emit(
            "parameters[left Int right Double]\n\
             add[left right]\n\
             add[(1 2) iota[left]]\n",
        );
        assert!(source.contains("fw_impl_10(args, 2U"));
        assert!(source.contains("static const int conversions[] = {1,0,0};"));
        assert!(source.contains("static const size_t shape_checks[] = {1U,0U};"));
        assert!(source.contains("\"left\",\"right\",\"\""));
        let spans = source
            .lines()
            .find(|line| line.starts_with("const size_t fw_parameter_spans"))
            .unwrap_or("");
        assert!(source.contains("{12U,1U,12U,20U,1U,20U}"), "{spans}");
        assert!(source.contains("{21U,1U,21U,33U,1U,33U}"), "{spans}");
        assert!(!source.contains("static_expression_type"));
        assert!(!source.contains("known_vector_length"));
        assert!(!source.contains("fw_apply("));
    }

    #[test]
    fn verified_generator_emits_constants_tuples_prefix_spread_and_fan_out() {
        let source = emit(
            "parameters[count Int]\n\
             [true 2 3.5]\n\
             add [1 2]\n\
             fanout[iota[count] {inc[_]} {add[_ 10]}]\n",
        );
        assert!(source.contains("fw_set_bool(out, 1)"));
        assert!(source.contains("fw_set_double(out, fw_double_from_bits"));
        assert!(source.contains("fw_make_tuple(out, 3U, \"tuple_literal\""));
        assert!(source.contains("FWV spread = {0}"));
        assert!(source.contains("fw_borrow(&spread.items[0U]"));
        assert!(source.contains("fw_ir_node_"));
        assert!(source.contains("(&operand, &out->items[0U])"));
        assert!(source.contains("fw_impl_34(args, 1U"));
    }

    #[test]
    fn verified_generation_is_byte_identical() {
        let program =
            match compile_source_with_name("parameters[n Int]\ninc[iota[n]]\n", "<source>") {
                Ok(program) => program,
                Err(error) => panic!("test source did not lower: {error}"),
            };
        let configuration = EvaluationConfiguration::default();
        let first = match emit_verified_c_program(&program, configuration) {
            Ok(emission) => emission.source,
            Err(error) => panic!("first generation failed: {error}"),
        };
        let second = match emit_verified_c_program(&program, configuration) {
            Ok(emission) => emission.source,
            Err(error) => panic!("second generation failed: {error}"),
        };
        assert_eq!(first.as_bytes(), second.as_bytes());
    }

    #[test]
    fn public_emission_uses_the_verified_backend_for_every_parameter_count() {
        let configuration = EvaluationConfiguration::default();
        for source in [
            "inc[1]\n",
            "inc[9223372036854775807]\n",
            "parameters[value Int]\ninc[value]\n",
        ] {
            let program = match compile_source_with_name(source, "<source>") {
                Ok(program) => program,
                Err(error) => panic!("test source did not lower: {error}"),
            };
            let expected = match emit_verified_c_program(&program, configuration) {
                Ok(emission) => emission,
                Err(error) => panic!("verified program did not emit: {error}"),
            };
            let public = match emit_c_source_with_configuration(source, configuration) {
                Ok(emission) => emission,
                Err(error) => panic!("public source did not emit: {error}"),
            };
            assert_eq!(
                public.source.as_bytes(),
                expected.source.as_bytes(),
                "{source}"
            );
            assert!(
                public
                    .source
                    .contains("/* VerifiedProgram-driven definitions. */")
            );
        }
    }

    #[test]
    fn verified_generator_preflights_configuration_and_tuple_profile() {
        let scalar = match compile_source_with_name("inc[1]\n", "<source>") {
            Ok(program) => program,
            Err(error) => panic!("scalar source did not lower: {error}"),
        };
        let invalid_configuration = EvaluationConfiguration {
            limits: crate::ResourceLimits {
                max_work_units: Some(1),
                ..crate::ResourceLimits::default()
            },
            ..EvaluationConfiguration::default()
        };
        let error = match emit_verified_c_program(&scalar, invalid_configuration) {
            Ok(_) => panic!("trusted profile with a limit unexpectedly emitted"),
            Err(error) => error,
        };
        assert_eq!(error.kind, ErrorKind::InvalidExecutionProfile);

        let tuple = match compile_source_with_name("parameters[x Int]\n[inc[x] 2]\n", "<source>") {
            Ok(program) => program,
            Err(error) => panic!("tuple source did not lower: {error}"),
        };
        let error = match emit_verified_c_program(
            &tuple,
            EvaluationConfiguration {
                profile: crate::ExecutionProfile::TrustedLocalV1,
                ..EvaluationConfiguration::default()
            },
        ) {
            Ok(_) => panic!("V1 tuple program unexpectedly emitted"),
            Err(error) => error,
        };
        assert_eq!(error.kind, ErrorKind::ProfileError);
        assert_eq!((error.location.line, error.location.column), (2, 1));
    }

    #[test]
    fn tuple_children_precede_outer_table_admission_in_generated_code() {
        let source = emit("[iota[3] iota[2]]\n");
        let tuple = match source.find("static int fw_ir_node_4") {
            Some(index) => &source[index..],
            None => panic!("missing tuple node"),
        };
        let first_child = tuple.find("fw_ir_node_1(hole, &children[0U])");
        let second_child = tuple.find("fw_ir_node_3(hole, &children[1U])");
        let admission = tuple.find("fw_make_tuple(out, 2U");
        assert!(
            matches!(
                (first_child, second_child, admission),
                (Some(first), Some(second), Some(admit)) if first < second && second < admit
            ),
            "tuple node did not execute children before admission"
        );
    }

    #[test]
    fn every_selected_id_emits_a_direct_kernel_symbol_without_type_redispatch() {
        let source = emit(
            "inc[1]\ninc[1.5]\ndec[1]\ndec[1.5]\nneg[1]\nneg[1.5]\n\
             abs[-1]\nabs[-1.5]\nadd[1 2]\nadd[1.0 2.0]\nsub[2 1]\nsub[2.0 1.0]\n\
             mul[2 3]\nmul[2.0 3.0]\nequals[true false]\nequals[1 2]\nequals[1.0 2.0]\n\
             not_equals[true false]\nnot_equals[1 2]\nnot_equals[1.0 2.0]\nnot[true]\n\
             and[true false]\nor[true false]\nodd[3]\neven[4]\nis_positive[1]\n\
             is_positive[1.0]\nis_negative[-1]\nis_negative[-1.0]\nless_than[1 2]\n\
             less_than[1.0 2.0]\ngreater_than[2 1]\ngreater_than[2.0 1.0]\niota[3]\n",
        );
        for implementation in 1..34 {
            assert!(
                source.contains(&format!("static int fw_kernel_{implementation}(")),
                "missing kernel {implementation}"
            );
            assert!(
                source.contains(&format!("fw_apply_selected(fw_kernel_{implementation},")),
                "implementation {implementation} is not direct"
            );
        }
        assert!(source.contains("static int fw_impl_34("));
        assert!(source.contains("return fw_apply_selected_iota(\"iota\""));
        assert!(!source.contains("fw_apply_scalar"));
        assert!(!source.contains("primitive=="));
    }

    #[test]
    fn operation_reference_identity_prepares_direct_c_dispatch_without_name_lookup() {
        let reference = must(crate::lowering::resolve_operation_reference(
            "add",
            crate::SourceSpan {
                begin: crate::SourceLocation::start(),
                end: crate::SourceLocation {
                    offset: 4,
                    line: 1,
                    column: 4,
                },
            },
            crate::OriginIndex(0),
            crate::lowering::OperationReferenceConstraint {
                parameter_types: &[Some(crate::ScalarType::Int), Some(crate::ScalarType::Int)],
                result_type: Some(crate::ScalarType::Int),
            },
            &mut crate::lowering::DiagnosticReservations::default(),
        ));
        let source = emit("add[1 2]\n");
        assert!(source.contains(&format!(
            "static int fw_kernel_{}(",
            reference.implementation_id
        )));
        assert!(source.contains(&format!(
            "static int fw_impl_{}(",
            reference.implementation_id
        )));
        assert!(!source.contains("primitive=="));
    }

    #[cfg(not(windows))]
    fn compile_and_run(source: &str, arguments: &[&str]) -> std::process::Output {
        compile_and_run_with_diagnostics(source, arguments, true)
    }

    #[cfg(not(windows))]
    fn compile_and_run_with_diagnostics(
        source: &str,
        arguments: &[&str],
        warnings_as_errors: bool,
    ) -> std::process::Output {
        use std::fs;
        use std::process::Command;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(error) => panic!("test clock is invalid: {error}"),
        };
        let directory =
            std::env::temp_dir().join(format!("faraweave-ir-c11-{}-{nonce}", std::process::id()));
        if let Err(error) = fs::create_dir(&directory) {
            panic!("unable to create test directory: {error}");
        }
        let c_path = directory.join("program.c");
        let executable = directory.join("program");
        if let Err(error) = fs::write(&c_path, source) {
            panic!("unable to write generated C: {error}");
        }
        let mut compiler = Command::new("cc");
        compiler.args([
            "-std=c11",
            "-frounding-math",
            "-ffp-contract=off",
            "-fno-fast-math",
            "-Wall",
            "-Wextra",
            "-pedantic-errors",
        ]);
        if warnings_as_errors {
            compiler.arg("-Werror");
        }
        let compile = match compiler
            .arg(&c_path)
            .arg("-o")
            .arg(&executable)
            .arg("-lm")
            .output()
        {
            Ok(output) => output,
            Err(error) => panic!("unable to launch strict C11 compiler: {error}"),
        };
        if !compile.status.success() {
            panic!(
                "strict C11 compilation failed:\n{}",
                String::from_utf8_lossy(&compile.stderr)
            );
        }
        let output = match Command::new(&executable).args(arguments).output() {
            Ok(output) => output,
            Err(error) => panic!("unable to execute generated program: {error}"),
        };
        if let Err(error) = fs::remove_dir_all(&directory) {
            panic!("unable to remove test directory: {error}");
        }
        output
    }

    #[cfg(not(windows))]
    fn assert_public_generated_matches_direct(
        source: &str,
        arguments: &[&str],
        configuration: EvaluationConfiguration,
    ) {
        let program = match compile_source_with_name(source, "<source>") {
            Ok(program) => program,
            Err(error) => panic!("test source did not lower: {error}"),
        };
        let internal = match emit_verified_c_program(&program, configuration) {
            Ok(emission) => emission.source,
            Err(error) => panic!("verified program did not emit: {error}"),
        };
        let public = match emit_c_source_with_configuration(source, configuration) {
            Ok(emission) => emission.source,
            Err(error) => panic!("public source did not emit: {error}"),
        };
        assert_eq!(public.as_bytes(), internal.as_bytes(), "{source}");

        let generated = compile_and_run(&public, arguments);
        let decoded = match crate::interpreter::decode_verified_arguments(&program, arguments) {
            Ok(arguments) => arguments,
            Err(error) => panic!("test arguments did not decode: {error}"),
        };
        match crate::evaluate_verified_program(&program, &decoded, configuration) {
            Ok(result) => {
                let mut expected = String::new();
                for value in &result.values {
                    match crate::format_value(value) {
                        Ok(value) => {
                            expected.push_str(&value);
                            expected.push('\n');
                        }
                        Err(error) => panic!("direct result did not format: {error}"),
                    }
                }
                assert!(generated.status.success(), "{source}");
                assert_eq!(generated.stdout, expected.as_bytes(), "{source}");
                assert!(generated.stderr.is_empty(), "{source}");
            }
            Err(error) => {
                let expected = format!(
                    "<generated>:{}:{}: {}: {}\n",
                    error.location.line,
                    error.location.column,
                    error.kind.diagnostic_name(),
                    error.message
                );
                assert!(!generated.status.success(), "{source}");
                assert!(generated.stdout.is_empty(), "{source}");
                assert_eq!(generated.stderr, expected.as_bytes(), "{source}");
            }
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn verified_generated_c_compiles_strictly_and_executes_success_and_failure_paths() {
        let successful = emit(
            "parameters[count Int]\n\
             add[count 2.0]\n\
             inc[(1 2 3)]\n\
             add [1 2]\n\
             fanout[iota[count] {inc[_]} {add[_ 10]}]\n",
        );
        let output = compile_and_run(&successful, &["3"]);
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "5.0\n(2 3 4)\n3\n[(2 3 4) (11 12 13)]\n"
        );
        assert!(output.stderr.is_empty());

        let failing = emit("parameters[count Int]\nadd[iota[count] iota[2]]\n");
        let output = compile_and_run(&failing, &["3"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("ShapeMismatch: add argument 2 expected shape [3], got [2]")
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn public_generated_c_matches_direct_ir_for_success_and_failure_corpus() {
        let configuration = EvaluationConfiguration {
            profile: crate::ExecutionProfile::TrustedLocalV2,
            ..EvaluationConfiguration::default()
        };
        let corpus: &[(&str, &[&str])] = &[
            ("parameters[x Int]\nadd[x 2.0]\n", &["3"]),
            (
                "parameters[n Int]\nadd [1 2]\nfanout[iota[n] {inc[_]} {add[_ 10]}]\n",
                &["3"],
            ),
            ("parameters[x Int]\ninc[x]\n", &["9223372036854775807"]),
            ("parameters[n Int]\nadd[iota[n] iota[2]]\n", &["3"]),
        ];
        for (source, arguments) in corpus {
            assert_public_generated_matches_direct(source, arguments, configuration);
        }

        let resource_configuration = EvaluationConfiguration {
            profile: crate::ExecutionProfile::BoundedV2,
            limits: crate::ResourceLimits {
                max_vector_bytes: Some(16),
                ..crate::ResourceLimits::default()
            },
            ..EvaluationConfiguration::default()
        };
        let source = "parameters[n Int]\n1\niota[n]\n";
        assert_public_generated_matches_direct(source, &["3"], resource_configuration);
    }

    #[cfg(not(windows))]
    #[test]
    fn tuple_resource_and_fault_order_matches_direct_ir() {
        let source = "[iota[3] iota[2]]\n";
        let configurations = [
            EvaluationConfiguration {
                profile: crate::ExecutionProfile::BoundedV2,
                limits: crate::ResourceLimits {
                    max_vector_bytes: Some(16),
                    ..crate::ResourceLimits::default()
                },
                ..EvaluationConfiguration::default()
            },
            EvaluationConfiguration {
                profile: crate::ExecutionProfile::BoundedV2,
                limits: crate::ResourceLimits {
                    max_live_evaluation_bytes: Some(32),
                    ..crate::ResourceLimits::default()
                },
                ..EvaluationConfiguration::default()
            },
            EvaluationConfiguration {
                profile: crate::ExecutionProfile::BoundedV2,
                limits: crate::ResourceLimits {
                    max_work_units: Some(4),
                    ..crate::ResourceLimits::default()
                },
                ..EvaluationConfiguration::default()
            },
            EvaluationConfiguration {
                profile: crate::ExecutionProfile::BoundedV2,
                limits: crate::ResourceLimits {
                    max_tuple_table_bytes: Some(16),
                    ..crate::ResourceLimits::default()
                },
                ..EvaluationConfiguration::default()
            },
            EvaluationConfiguration {
                profile: crate::ExecutionProfile::BoundedV2,
                limits: crate::ResourceLimits {
                    max_vector_bytes: Some(128),
                    max_tuple_table_bytes: Some(128),
                    max_live_evaluation_bytes: Some(256),
                    max_work_units: Some(256),
                },
                allocation_failure: crate::AllocationFailureInjection {
                    fail_at_ordinal: Some(0),
                },
            },
            EvaluationConfiguration {
                profile: crate::ExecutionProfile::BoundedV2,
                limits: crate::ResourceLimits {
                    max_vector_bytes: Some(128),
                    max_tuple_table_bytes: Some(128),
                    max_live_evaluation_bytes: Some(256),
                    max_work_units: Some(256),
                },
                allocation_failure: crate::AllocationFailureInjection {
                    fail_at_ordinal: Some(1),
                },
            },
            EvaluationConfiguration {
                profile: crate::ExecutionProfile::BoundedV2,
                limits: crate::ResourceLimits {
                    max_vector_bytes: Some(128),
                    max_tuple_table_bytes: Some(128),
                    max_live_evaluation_bytes: Some(256),
                    max_work_units: Some(256),
                },
                allocation_failure: crate::AllocationFailureInjection {
                    fail_at_ordinal: Some(2),
                },
            },
        ];
        for configuration in configurations {
            assert_public_generated_matches_direct(source, &[], configuration);
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn all_direct_selected_kernels_compile_strictly_and_match_direct_ir() {
        let source = "inc[1]\ninc[1.5]\ndec[1]\ndec[1.5]\nneg[1]\nneg[1.5]\n\
             abs[-1]\nabs[-1.5]\nadd[1 2]\nadd[1.0 2.0]\nsub[2 1]\nsub[2.0 1.0]\n\
             mul[2 3]\nmul[2.0 3.0]\nequals[true false]\nequals[1 2]\nequals[1.0 2.0]\n\
             not_equals[true false]\nnot_equals[1 2]\nnot_equals[1.0 2.0]\nnot[true]\n\
             and[true false]\nor[true false]\nodd[3]\neven[4]\nis_positive[1]\n\
             is_positive[1.0]\nis_negative[-1]\nis_negative[-1.0]\nless_than[1 2]\n\
             less_than[1.0 2.0]\ngreater_than[2 1]\ngreater_than[2.0 1.0]\niota[3]\n";
        let configuration = EvaluationConfiguration::default();
        assert_public_generated_matches_direct(source, &[], configuration);
    }
}
