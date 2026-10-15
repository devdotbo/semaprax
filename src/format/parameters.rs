use super::{contains_record_construction, write_expr, write_type, Expr};

pub(crate) fn write_record_literal_delimited_expr(output: &mut impl std::fmt::Write, value: &Expr) {
    let delimited = contains_record_construction(value);
    if delimited {
        output.write_char('(').unwrap();
    }
    write_expr(output, value, 0);
    if delimited {
        output.write_char(')').unwrap();
    }
}

pub(crate) fn write_type_parameters(
    output: &mut impl std::fmt::Write,
    parameters: &[crate::ast::TypeParameterDeclaration],
) {
    if parameters.is_empty() {
        return;
    }
    output.write_char('<').unwrap();
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            output.write_str(", ").unwrap();
        }
        output.write_str(&parameter.name).unwrap();
    }
    output.write_char('>').unwrap();
}

pub(super) fn write_type_arguments(
    output: &mut impl std::fmt::Write,
    arguments: &[crate::ast::Type],
) {
    if arguments.is_empty() {
        return;
    }
    output.write_char('<').unwrap();
    for (index, argument) in arguments.iter().enumerate() {
        if index != 0 {
            output.write_str(", ").unwrap();
        }
        write_type(output, argument);
    }
    output.write_char('>').unwrap();
}
