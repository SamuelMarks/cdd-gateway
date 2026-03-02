# Implementing `to_docs_json` in `cdd-*` Repositories

This document contains a comprehensive prompt that you can copy and paste to any LLM working on your other `cdd-*` repositories (like Java, Python, Go, etc.). It provides them with the context, the exact JSON schema, and the implementation requirements to build the `to_docs_json` feature consistently across your ecosystem.

---

## The Prompt to Copy

```text
# Objective
You are assisting me in extending one of my `cdd-*` (Code-Driven Development) repositories. These repositories generate API clients from OpenAPI specifications for various programming languages.

I am building a central API documentation site that takes an OpenAPI spec as input and produces "how to call" code examples for every language in the `cdd-*` ecosystem.

Your task is to implement a new CLI subcommand named `to_docs_json` in this repository. This command will parse an OpenAPI file and output a specific JSON structure containing idiomatic code examples for calling each API endpoint in this repository's target language/framework.

# Requirements

## 1. The Output JSON Schema
The `to_docs_json` command MUST output a JSON array of objects to `stdout` matching the following JSON schema:

{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "array",
  "description": "A collection of API documentation code examples for different programming languages or frameworks.",
  "items": {
    "type": "object",
    "properties": {
      "language": {
        "type": "string",
        "description": "The programming language or framework (e.g., typescript, angular, fetch, java, python)"
      },
      "operations": {
        "type": "array",
        "description": "List of operations (API endpoints) and their corresponding code examples.",
        "items": {
          "type": "object",
          "properties": {
            "method": {
              "type": "string",
              "description": "HTTP method (e.g., GET, POST)"
            },
            "path": {
              "type": "string",
              "description": "API endpoint path"
            },
            "operationId": {
              "type": "string",
              "description": "The operation ID from the OpenAPI specification"
            },
            "code": {
              "type": "object",
              "description": "Structured representation of the generated code example. This structure allows doc site implementors to offer UI toggles for 'Imports' and 'Wrapping' (e.g., class/function boilerplate), enabling a fallback to a very concise syntax example ('snippet') for how to call the API in this language.",
              "properties": {
                "imports": {
                  "type": "string",
                  "description": "The import statements required for the code example. Can be toggled on/off in the documentation site."
                },
                "wrapper_start": {
                  "type": "string",
                  "description": "The opening boilerplate code, such as a class definition, 'public static void main(String[] args) {', or an async function declaration. Can be toggled on/off."
                },
                "snippet": {
                  "type": "string",
                  "description": "The core, concise syntax example showing the actual API call logic (e.g., HTTP GET /pet)."
                },
                "wrapper_end": {
                  "type": "string",
                  "description": "The closing boilerplate code (e.g., closing braces for the wrapper_start). Can be toggled on/off."
                }
              },
              "required": ["snippet"]
            }
          },
          "required": ["method", "path", "code"]
        }
      }
    },
    "required": ["language", "operations"]
  }
}

## 2. The CLI Command structure
Add the subcommand `to_docs_json` to the CLI tool for this repository. It must accept the following flags:
*   `-i, --input <path>` (Required): Path or URL to the OpenAPI specification.
*   `--no-imports` (Optional): If provided, omit the `imports` field in the `code` object.
*   `--no-wrapping` (Optional): If provided, omit the `wrapper_start` and `wrapper_end` fields in the `code` object.

## 3. Code Generation Rules
You must implement a generator that visits every operation in the OpenAPI spec and produces the four components of the `code` object.

*   **`imports`**: Should contain the necessary `import`, `using`, or `require` statements to make the code run.
*   **`wrapper_start` / `wrapper_end`**: Some languages (like Java or C#) require significant boilerplate to execute code (e.g., `public class Example { public static void main(...) { ...`). Put the opening of this boilerplate in `wrapper_start` and the closing in `wrapper_end`. If the language supports top-level scripts (like Python or JS), this might just be an `async def main():` or similar wrapper, or it could be empty if no wrapper is strictly needed.
*   **`snippet`**: This is the most crucial part. It MUST contain the actual initialization of the client/service and the execution of the endpoint call (e.g., `PetService client = new PetService(); client.getPet(123);`). If the user toggles OFF wrapping and imports, this snippet should be self-sufficient enough to understand the exact syntax of the core API call.

## 4. Execution Steps
1.  Read the OpenAPI spec from the input argument.
2.  Iterate through all parsed paths and operations.
3.  For each operation, format the code structure based on the target language's idiomatic client usage.
4.  If `--no-imports` is passed, do not populate `code.imports`.
5.  If `--no-wrapping` is passed, do not populate `code.wrapper_start` and `code.wrapper_end`.
6.  Print the resulting JSON array of language(s) directly to `stdout` with formatting (e.g., `JSON.stringify(..., null, 2)` or equivalent). Do not output other logging to `stdout`, as this CLI command's output will be piped into other tools. Output any errors or warnings to `stderr`.

## 5. Review & Testing
1. Ensure your implementation accurately reflects how the code is called in the target language.
2. Provide tests to verify the `to_docs_json` command logic, particularly verifying that the `--no-imports` and `--no-wrapping` toggles properly omit those keys from the output JSON objects.
3. Build and ensure all existing checks pass.

Please review the codebase of this repository, implement this feature seamlessly into the existing CLI structure, and let me know when it is ready.
```
