// Module declarations
pub mod amplify;
pub mod aws_cli;
pub mod build;
pub mod command;
pub mod file_ops;
pub mod git_api;
pub mod git_ops;
pub mod prerequisites;
pub mod runtime;

// Re-export commands for registration
use amplify::{
    amplify_env_checkout, amplify_env_checkout_streaming, amplify_pull, amplify_pull_streaming,
    delete_gen2_sandbox, deploy_gen2_sandbox, update_gen2_build_config, upgrade_amplify_cli,
};
use aws_cli::{
    get_amplify_job, get_aws_profiles, get_aws_regions, get_current_app_env_vars,
    get_current_branch_env_vars, get_lambda_functions, get_lambda_functions_with_status,
    get_latest_amplify_job, get_profile_region, list_amplify_apps, list_amplify_branches,
    list_amplify_jobs, revert_build_spec, start_amplify_job, update_app_env_vars,
    update_branch_env_vars, update_custom_image_env_var, update_live_updates_env_var,
};
use build::run_build;
use file_ops::{
    detect_backend_type, detect_package_manager, install_dependencies,
    install_dependencies_streaming, update_gen1_backend, update_gen2_backend,
    upgrade_amplify_backend_packages,
};
use git_api::check_branch_protection;
use git_ops::{cleanup_repository, clone_repository, commit_and_push};
use prerequisites::check_prerequisites;
use runtime::{get_supported_runtimes, get_target_runtime};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            check_prerequisites,
            get_supported_runtimes,
            get_target_runtime,
            get_aws_profiles,
            get_aws_regions,
            get_profile_region,
            list_amplify_apps,
            list_amplify_branches,
            list_amplify_jobs,
            get_amplify_job,
            start_amplify_job,
            get_latest_amplify_job,
            get_lambda_functions,
            get_lambda_functions_with_status,
            clone_repository,
            commit_and_push,
            cleanup_repository,
            detect_package_manager,
            detect_backend_type,
            install_dependencies,
            install_dependencies_streaming,
            amplify_pull,
            amplify_pull_streaming,
            amplify_env_checkout,
            amplify_env_checkout_streaming,
            update_gen1_backend,
            update_gen2_backend,
            update_gen2_build_config,
            upgrade_amplify_cli,
            upgrade_amplify_backend_packages,
            run_build,
            deploy_gen2_sandbox,
            delete_gen2_sandbox,
            update_live_updates_env_var,
            update_custom_image_env_var,
            update_app_env_vars,
            update_branch_env_vars,
            get_current_app_env_vars,
            get_current_branch_env_vars,
            check_branch_protection,
            revert_build_spec
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
