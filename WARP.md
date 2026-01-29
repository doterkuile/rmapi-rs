Implementation Plan - Add 
get
 command
The goal is to implement the 
get
 command in rmapi-rs to download files from the ReMarkable cloud. Supported formats:

PDF -> 
.pdf
EPUB -> .epub
Notebooks -> .rmdoc (Zip of all blobs)
Folders -> Recursive download
User Review Required
Breaking Changes: None.
Dependencies: Adding zip = "0.6" to 
rmapi/Cargo.toml
.
Proposed Changes
Subtask 1: Dependencies & Core API
 
rmapi/Cargo.toml
: Add zip = "0.6" dependency.
 
rmapi/src/endpoints.rs
: Ensure 
fetch_blob
 is public or create download_blob wrapper if needed.
Subtask 2: Document Download Logic (Library)
 
rmapi/src/client.rs
: Implement download_document(doc_id, target_path).
 Fetch docSchema to get file list.
 Detect file type.
 Implement PDF/EPUB single-file download.
 Implement .rmdoc zip creation for native files.
Subtask 3: CLI Command Implementation
 
rmclient/src/rmclient/commands.rs
: Add Get variant to Commands enum with optional -r flag.
 
rmclient/src/main.rs
: Implement logic for Commands::Get.
 Handle single file download.
 Implement recursive folder traversal and download.
Subtask 4: Interactive Shell Integration
 
rmclient/src/rmclient/shell.rs
: Add Get to ShellCommand enum.
 
rmclient/src/rmclient/shell.rs
: Implement exec_get.
 Use 
normalize_path
 to resolve inputs.
 Reuse recursive download logic from 
main.rs
 if possible, or duplicate/move to shared helper.
Verification Plan
Automated Tests
Unit tests for new helper functions if complex (e.g. zip packaging, though that involves IO).
Manual Verification
Setup: Ensure authentication is working (rmclient ls).
PDF Download: rmclient get "My PDF" -> Check My PDF.pdf exists.
EPUB Download: rmclient get "My Book" -> Check My Book.epub exists.
Notebook Download: rmclient get "My Notes" -> Check My Notes.rmdoc exists and is a valid zip.
Folder Download: rmclient get -r "My Folder" -> Check recursive download.
Interactive Shell:
Run rmclient shell.
cd
 into a folder.
get
 a file using relative path.