use v0_bash_agent::{execute_bash, get_bash_tool, get_cwd, get_system_prompt};

#[test]
fn test_integration_bash_commands() {
    // Test multiple bash commands in sequence
    let result = execute_bash("echo 'test' > /tmp/v0_test.txt && cat /tmp/v0_test.txt");
    assert!(result.contains("test"));

    // Cleanup
    execute_bash("rm -f /tmp/v0_test.txt");
}

#[test]
fn test_bash_tool_structure() {
    let tool = get_bash_tool();

    // Verify tool structure
    assert_eq!(tool.name, "bash");
    assert!(!tool.description.is_empty());

    // Verify input schema has required fields
    let schema = tool.input_schema;
    assert!(schema.get("type").is_some());
    assert!(schema.get("properties").is_some());
    assert!(schema.get("required").is_some());

    // Verify required contains "command"
    if let Some(required) = schema.get("required") {
        if let Some(required_array) = required.as_array() {
            assert!(required_array.iter().any(|v| v.as_str() == Some("command")));
        }
    }
}

#[test]
fn test_system_prompt_contains_required_info() {
    let prompt = get_system_prompt();

    // Verify essential components
    assert!(prompt.contains("CLI agent"));
    assert!(prompt.contains("bash commands"));
    assert!(prompt.contains("cat"));
    assert!(prompt.contains("grep"));
    assert!(prompt.contains("echo"));
    assert!(prompt.contains("v0_bash_agent"));
}

#[test]
fn test_cwd_is_valid() {
    let cwd = get_cwd();

    // Should be a valid path
    assert!(!cwd.is_empty());
    assert!(cwd.starts_with("/") || cwd.contains(":"));
}

#[test]
fn test_execute_bash_file_operations() {
    let test_file = "/tmp/v0_bash_agent_test.txt";
    let test_content = "Hello from integration test";

    // Write to file
    let write_cmd = format!("echo '{}' > {}", test_content, test_file);
    execute_bash(&write_cmd);

    // Read from file
    let read_cmd = format!("cat {}", test_file);
    let result = execute_bash(&read_cmd);
    assert!(result.contains(test_content));

    // Cleanup
    execute_bash(&format!("rm -f {}", test_file));
}

#[test]
fn test_execute_bash_with_environment_variables() {
    let result = execute_bash("export TEST_VAR=hello && echo $TEST_VAR");
    assert!(result.contains("hello"));
}

#[test]
fn test_execute_bash_multiline_script() {
    let script = r#"
        x=5
        y=10
        echo $((x + y))
    "#;
    let result = execute_bash(script);
    assert!(result.contains("15"));
}

#[test]
fn test_execute_bash_grep_pattern() {
    let result = execute_bash("echo -e 'line1\nline2\nline3' | grep 'line2'");
    assert!(result.contains("line2"));
    assert!(!result.contains("line1") || result.contains("line1\nline2"));
}

#[test]
fn test_execute_bash_find_files() {
    // Create a test directory structure
    execute_bash("mkdir -p /tmp/v0_test_dir");
    execute_bash("touch /tmp/v0_test_dir/test.txt");

    let result = execute_bash("find /tmp/v0_test_dir -name '*.txt'");
    assert!(result.contains("test.txt"));

    // Cleanup
    execute_bash("rm -rf /tmp/v0_test_dir");
}

#[test]
fn test_execute_bash_error_handling() {
    let result = execute_bash("ls /nonexistent_directory_12345");
    // Should contain error message but not crash
    assert!(
        result.contains("No such file or directory")
            || result.contains("cannot access")
            || result.contains("not found")
    );
}
