//! Retained Project effect and permit admission, with unchanged closed profiles.
use super::*;

pub(in crate::workspace_graph) fn project_effects_admitted(
    profile: crate::project::ProjectProfile,
    effects: &[String],
    natives: &ScalarNativeImports,
) -> bool {
    if profile.is_source_command() {
        return effects
            .iter()
            .all(|v| crate::source_command::EFFECTS.contains(&v.as_str()));
    }
    (profile == crate::project::ProjectProfile::EnvironmentIoV1
        && effects.iter().all(|effect| {
            crate::project::PROJECT_ENVIRONMENT_CAPABILITIES_V1.contains(&effect.as_str())
        }))
        || (profile == crate::project::ProjectProfile::ProcessIoV1
            && effects.iter().all(|effect| {
                crate::project::PROJECT_PROCESS_CAPABILITIES_V1.contains(&effect.as_str())
            }))
        || (profile.is_filesystem()
            && effects
                .iter()
                .all(|effect| crate::filesystem_ops::FILESYSTEM_EFFECTS.contains(&effect.as_str())))
        || effects.is_empty()
        || natives.effects_admitted(effects)
        || (matches!(
            profile,
            crate::project::ProjectProfile::UsefulDataCommandV1
                | crate::project::ProjectProfile::UsefulDataCommandV2
        ) && effects == [crate::host_io_ops::STDOUT_WRITE_EFFECT])
        || (matches!(
            profile,
            crate::project::ProjectProfile::LanguageCommandIoV1
                | crate::project::ProjectProfile::StdinStreamCommandIoV1
                | crate::project::ProjectProfile::StdinStreamCommandIoV2
                | crate::project::ProjectProfile::StdinStreamTextCommandIoV1
                | crate::project::ProjectProfile::StdinStreamDataCommandIoV1
                | crate::project::ProjectProfile::StdinStreamDataCommandIoV2
                | crate::project::ProjectProfile::StdinStreamOwnedDataCommandIoV1
                | crate::project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1
                | crate::project::ProjectProfile::StdinStreamNestedOutcomeCommandIoV1
                | crate::project::ProjectProfile::LineCommandIoV1
        ) && effects.iter().all(|effect| {
            matches!(
                effect.as_str(),
                crate::command_io_ops::ARGS_READ_EFFECT
                    | crate::command_io_ops::STDERR_WRITE_EFFECT
                    | crate::command_io_ops::STDIN_READ_EFFECT
                    | crate::host_io_ops::STDOUT_WRITE_EFFECT
            )
        }))
        || (profile == crate::project::ProjectProfile::NetworkCommandIoV1
            && effects.iter().all(|effect| {
                crate::project::PROJECT_NETWORK_COMMAND_CAPABILITIES_V1.contains(&effect.as_str())
            }))
        || (profile == crate::project::ProjectProfile::HttpsCommandIoV1
            && effects.iter().all(|effect| {
                crate::project::PROJECT_HTTPS_COMMAND_CAPABILITIES_V1.contains(&effect.as_str())
            }))
}

/// Whether one retained module's declared permits stay inside the Project
/// profile's admitted authority.
pub(in crate::workspace_graph) fn permits_admitted(
    profile: crate::project::ProjectProfile,
    module: &WorkspaceResolvedModule,
    entry_module: &str,
    natives: &ScalarNativeImports,
) -> bool {
    if profile.is_source_command() {
        return module
            .permits
            .iter()
            .all(|v| crate::source_command::EFFECTS.contains(&v.as_str()));
    }
    if profile.is_stdin_stream() {
        return module.permits.iter().all(|permit| {
            crate::project::PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2.contains(&permit.as_str())
        });
    }
    (profile == crate::project::ProjectProfile::EnvironmentIoV1
        && module.permits.iter().all(|effect| {
            crate::project::PROJECT_ENVIRONMENT_CAPABILITIES_V1.contains(&effect.as_str())
        }))
        || (profile == crate::project::ProjectProfile::ProcessIoV1
            && module.permits.iter().all(|effect| {
                crate::project::PROJECT_PROCESS_CAPABILITIES_V1.contains(&effect.as_str())
            }))
        || (profile.is_filesystem()
            && module
                .permits
                .iter()
                .all(|effect| crate::filesystem_ops::FILESYSTEM_EFFECTS.contains(&effect.as_str())))
        || module.permits.is_empty()
        || natives.effects_admitted(&module.permits)
        || (matches!(
            profile,
            crate::project::ProjectProfile::UsefulDataCommandV1
                | crate::project::ProjectProfile::UsefulDataCommandV2
        ) && module.module == entry_module
            && module.permits == [crate::host_io_ops::STDOUT_WRITE_EFFECT])
        || (matches!(
            profile,
            crate::project::ProjectProfile::LanguageCommandIoV1
                | crate::project::ProjectProfile::StdinStreamCommandIoV1
                | crate::project::ProjectProfile::StdinStreamCommandIoV2
                | crate::project::ProjectProfile::StdinStreamTextCommandIoV1
                | crate::project::ProjectProfile::StdinStreamDataCommandIoV1
                | crate::project::ProjectProfile::StdinStreamDataCommandIoV2
                | crate::project::ProjectProfile::StdinStreamOwnedDataCommandIoV1
                | crate::project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1
                | crate::project::ProjectProfile::StdinStreamNestedOutcomeCommandIoV1
                | crate::project::ProjectProfile::LineCommandIoV1
        ) && module.module == entry_module
            && module.permits
                == [
                    crate::command_io_ops::ARGS_READ_EFFECT,
                    crate::command_io_ops::STDERR_WRITE_EFFECT,
                    crate::command_io_ops::STDIN_READ_EFFECT,
                    crate::host_io_ops::STDOUT_WRITE_EFFECT,
                ])
        || (profile == crate::project::ProjectProfile::NetworkCommandIoV1
            && module.module == entry_module
            && module
                .permits
                .iter()
                .map(String::as_str)
                .eq(crate::project::PROJECT_NETWORK_COMMAND_CAPABILITIES_V1))
        || (profile == crate::project::ProjectProfile::HttpsCommandIoV1
            && module.module == entry_module
            && module
                .permits
                .iter()
                .map(String::as_str)
                .eq(crate::project::PROJECT_HTTPS_COMMAND_CAPABILITIES_V1))
}
