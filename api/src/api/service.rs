use crate::{
    config::{LambdoConfig, LambdoLanguageConfig},
    vm_manager::grpc_definitions::{
        ExecuteRequest, ExecuteRequestStep, ExecuteResponse, FileModel,
    },
    vm_manager::{state::LambdoStateRef, Error, VMManager},
};
use log::{debug, trace};
use uuid::Uuid;

use crate::model::RunRequest;

pub struct LambdoApiService {
    pub config: LambdoConfig,
    pub vm_manager: VMManager,
}

impl LambdoApiService {
    pub async fn new(config: LambdoConfig) -> Result<Self, Error> {
        let state = crate::vm_manager::state::LambdoState::new(config.clone());
        let vm_manager =
            VMManager::new(std::sync::Arc::new(tokio::sync::Mutex::new(state))).await?;
        Ok(LambdoApiService { config, vm_manager })
    }

    pub async fn new_with_state(state: LambdoStateRef) -> Result<Self, Error> {
        let config = state.lock().await.config.clone();
        let vm_manager = VMManager::new(state).await?;
        Ok(LambdoApiService { config, vm_manager })
    }

    pub async fn run_code(&self, request: RunRequest) -> Result<ExecuteResponse, Error> {
        let entrypoint = request.code[0].filename.clone();

        let language_settings = self.find_language(&request.language).unwrap();
        let steps = Self::generate_steps(&language_settings, &entrypoint);
        let file = FileModel {
            filename: entrypoint.to_string(),
            content: request.code[0].content.clone(),
        };
        let input_filename = "input.input";

        let input = FileModel {
            filename: input_filename.to_string(),
            content: request.input.clone(),
        };

        let request_data = ExecuteRequest {
            id: Uuid::new_v4().to_string(),
            steps,
            files: vec![file, input],
        };
        trace!("Request message to VMM: {:?}", request_data);

        let response = self
            .vm_manager
            .run_code(request_data, language_settings.into())
            .await;
        debug!("Response from VMM: {:?}", response);

        response
    }

    fn find_language(
        &self,
        language: &String,
    ) -> Result<LambdoLanguageConfig, Box<dyn std::error::Error>> {
        let language_list = &self.config.languages;
        for lang in language_list {
            if &*lang.name == language {
                return Ok(lang.clone());
            }
        }
        Err("Language not found".into())
    }

    fn generate_steps(
        language_settings: &LambdoLanguageConfig,
        entrypoint: &str,
    ) -> Vec<ExecuteRequestStep> {
        let mut steps: Vec<ExecuteRequestStep> = Vec::new();
        for step in &language_settings.steps {
            let command = step.command.replace("{{filename}}", entrypoint);

            steps.push(ExecuteRequestStep {
                command,
                enable_output: step.output.enabled,
            });
        }
        steps
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use tokio::sync::Mutex;

    use super::LambdoApiService;
    use crate::{
        config::{
            LambdoAgentConfig, LambdoApiConfig, LambdoConfig, LambdoLanguageConfig,
            LambdoLanguageStepConfig, LambdoLanguageStepOutputConfig, LambdoVMMConfig,
        },
        vm_manager::{state::LambdoState, VMManager},
    };

    fn generate_lambdo_test_config() -> LambdoConfig {
        LambdoConfig {
            apiVersion: "lambdo.io/v1alpha1".to_string(),
            kind: "Config".to_string(),
            api: LambdoApiConfig {
                web_host: "0.0.0.0".to_string(),
                web_port: 3000,
                grpc_host: "0.0.0.0".to_string(),
                gprc_port: 50051,
                bridge: "lambdo0".to_string(),
                bridge_address: "0.0.0.0".to_string(),
            },
            vmm: LambdoVMMConfig {
                kernel: "/var/lib/lambdo/kernel/vmlinux.bin".to_string(),
            },
            agent: LambdoAgentConfig {
                path: "/usr/local/bin/lambdo-agent".to_string(),
                config: "/etc/lambdo/agent.yaml".to_string(),
            },
            languages: vec![
                LambdoLanguageConfig {
                    name: "NODE".to_string(),
                    version: "1.0".to_string(),
                    initramfs: "test".to_string(),
                    steps: vec![
                        LambdoLanguageStepConfig {
                            name: Some("step 1".to_string()),
                            command: "echo {{filename}}".to_string(),
                            output: LambdoLanguageStepOutputConfig {
                                enabled: true,
                                debug: false,
                            },
                        },
                        LambdoLanguageStepConfig {
                            name: Some("step 2".to_string()),
                            command: "echo hello".to_string(),
                            output: LambdoLanguageStepOutputConfig {
                                enabled: true,
                                debug: false,
                            },
                        },
                        LambdoLanguageStepConfig {
                            name: Some("step 3".to_string()),
                            command: "cat {{filename}} > {{filename}}".to_string(),
                            output: LambdoLanguageStepOutputConfig {
                                enabled: true,
                                debug: false,
                            },
                        },
                    ],
                },
                LambdoLanguageConfig {
                    name: "PYTHON".to_string(),
                    version: "3.0".to_string(),
                    initramfs: "test".to_string(),
                    steps: vec![LambdoLanguageStepConfig {
                        name: Some("step".to_string()),
                        command: "echo {{filename}}".to_string(),
                        output: LambdoLanguageStepOutputConfig {
                            enabled: true,
                            debug: false,
                        },
                    }],
                },
            ],
        }
    }

    #[test]
    fn test_generate_steps() {
        let language_settings = LambdoLanguageConfig {
            name: "NODE".to_string(),
            version: "1.0".to_string(),
            initramfs: "test".to_string(),
            steps: generate_lambdo_test_config().languages[0].steps.clone(),
        };
        let entrypoint = "index.js";

        let expected_steps = vec![
            "echo index.js".to_string(),
            "echo hello".to_string(),
            "cat index.js > index.js".to_string(),
        ];

        let steps = LambdoApiService::generate_steps(&language_settings, &entrypoint);

        assert_eq!(steps.len(), 3);
        for (i, step) in steps.iter().enumerate() {
            assert_eq!(step.command, expected_steps[i]);
        }
    }

    #[test]
    fn test_find_language() {
        let config = generate_lambdo_test_config();
        let service = LambdoApiService {
            config: config.clone(),
            vm_manager: VMManager {
                state: Arc::new(Mutex::new(LambdoState::new(config))),
            },
        };

        let language = "NODE".to_string();
        let language_settings = service.find_language(&language).unwrap();

        assert_eq!(language_settings.name, language);
        assert_eq!(language_settings.steps[0].name, Some("step 1".to_string()));
    }
}
