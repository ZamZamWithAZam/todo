use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write, stdin, stdout};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::env;

const TODO_DIR: &str = ".todo_lists";

struct TodoApp {
    base_dir: PathBuf,
}

#[derive(Clone, Debug)]
struct TodoItem {
    text: String,
    tags: Vec<PathBuf>,
}

impl TodoItem {
    fn new(text: String) -> Self {
        TodoItem {
            text,
            tags: Vec::new(),
        }
    }

    fn to_string(&self) -> String {
        let mut result = self.text.clone();
        if !self.tags.is_empty() {
            result.push_str(" [TAGS:");
            result.push_str(&self.tags
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("|"));
            result.push_str("]");
        }
        result
    }

    fn from_string(s: &str) -> Self {
        if let Some(tag_start) = s.find(" [TAGS:") {
            if let Some(tag_end) = s.rfind("]") {
                let text = s[..tag_start].to_string();
                let tags_str = &s[tag_start + 8..tag_end];
                let tags = tags_str
                    .split('|')
                    .map(|t| PathBuf::from(t.trim()))
                    .collect();
                return TodoItem { text, tags };
            }
        }
        TodoItem::new(s.to_string())
    }

    fn add_tag(&mut self, new_tag: PathBuf) {
        self.tags.retain(|tag| tag != &new_tag);
        self.tags.push(new_tag);
    }
}

impl TodoApp {
    fn new() -> io::Result<Self> {
        let home = dirs::home_dir().expect("Could not find home directory");
        let base_dir = home.join(TODO_DIR);
        fs::create_dir_all(&base_dir)?;
        Ok(TodoApp { base_dir })
    }

    fn get_list_path(&self, list_name: &str) -> PathBuf {
        self.base_dir.join(format!("{}.txt", list_name))
    }

    fn add_task(&self, task: &str, list_name: &str) -> io::Result<()> {
        let file_path = self.get_list_path(list_name);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        let item = TodoItem::new(task.to_string());
        writeln!(file, "{}", item.to_string())?;
        println!("Task added to list '{}': {}", list_name, task);
        Ok(())
    }

    fn resolve_file_path(file_arg: Option<&str>) -> io::Result<PathBuf> {
        match file_arg {
            Some(filename) => {
                let current_dir = env::current_dir()?;
                println!("Current directory: {}", current_dir.display());
                println!("Filename argument: {}", filename);
                
                // If filename is given without path, prepend current directory
                if !filename.contains('/') {
                    let full_path = current_dir.join(filename.trim_matches('"'));  // Remove any quotes
                    println!("Full resolved path: {}", full_path.display());
                    Ok(full_path)
                } else {
                    Ok(PathBuf::from(filename))
                }
            },
            None => {
                env::current_dir()
            }
        }
    }

    fn add_tag(&self, file_arg: Option<&str>, task_num: usize, list_name: &str) -> io::Result<()> {
        let list_path = self.get_list_path(list_name);
        if !list_path.exists() {
            println!("List '{}' not found.", list_name);
            return Ok(());
        }

        // Debug the incoming argument
        println!("Received file argument: {:?}", file_arg);
        
        let resolved_path = Self::resolve_file_path(file_arg)?;
        println!("Resolved path before tagging: {}", resolved_path.display());

        let mut items: Vec<TodoItem> = BufReader::new(File::open(&list_path)?)
            .lines()
            .filter_map(Result::ok)
            .map(|line| TodoItem::from_string(&line))
            .collect();

        if task_num == 0 || task_num > items.len() {
            println!("Error: Invalid task number");
            return Ok(());
        }

        items[task_num - 1].add_tag(resolved_path.clone());
        
        let mut file = File::create(&list_path)?;
        for item in items {
            writeln!(file, "{}", item.to_string())?;
        }

        println!("Tagged task {} in list '{}' with '{}'", 
            task_num, 
            list_name, 
            resolved_path.display()
        );
        Ok(())
    }

    fn list_tasks(&self, list_name: &str) -> io::Result<()> {
        let file_path = self.get_list_path(list_name);
        if !file_path.exists() {
            println!("No tasks found in list '{}'.", list_name);
            return Ok(());
        }

        println!("Tasks in list '{}':", list_name);
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        for (index, line) in reader.lines().enumerate() {
            if let Ok(task) = line {
                let item = TodoItem::from_string(&task);
                println!("{}. {}", index + 1, item.to_string());
            }
        }
        Ok(())
    }

    fn list_all_lists(&self) -> io::Result<()> {
        println!("Available todo lists:");
        for entry in fs::read_dir(&self.base_dir)? {
            if let Ok(entry) = entry {
                if let Some(file_name) = entry.path().file_stem() {
                    if let Some(name) = file_name.to_str() {
                        println!("- {}", name);
                    }
                }
            }
        }
        Ok(())
    }

    fn remove_task(&self, task_num: usize, list_name: &str) -> io::Result<()> {
        let file_path = self.get_list_path(list_name);
        if !file_path.exists() {
            println!("List '{}' not found.", list_name);
            return Ok(());
        }

        let tasks: Vec<String> = BufReader::new(File::open(&file_path)?)
            .lines()
            .filter_map(Result::ok)
            .collect();

        if task_num == 0 || task_num > tasks.len() {
            println!("Error: Invalid task number");
            return Ok(());
        }

        let mut new_tasks = tasks.clone();
        new_tasks.remove(task_num - 1);

        let mut file = File::create(&file_path)?;
        for task in new_tasks {
            writeln!(file, "{}", task)?;
        }

        println!("Task {} removed from list '{}'", task_num, list_name);
        Ok(())
    }

    fn edit_task(&self, task_num: usize, new_text: &str, list_name: &str) -> io::Result<()> {
        let file_path = self.get_list_path(list_name);
        if !file_path.exists() {
            println!("List '{}' not found.", list_name);
            return Ok(());
        }

        let mut tasks: Vec<String> = BufReader::new(File::open(&file_path)?)
            .lines()
            .filter_map(Result::ok)
            .collect();

        if task_num == 0 || task_num > tasks.len() {
            println!("Error: Invalid task number");
            return Ok(());
        }

        tasks[task_num - 1] = new_text.to_string();

        let mut file = File::create(&file_path)?;
        for task in tasks {
            writeln!(file, "{}", task)?;
        }

        println!("Task {} updated in list '{}'", task_num, list_name);
        Ok(())
    }

    fn get_available_lists(&self) -> io::Result<Vec<String>> {
        let mut lists = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            if let Ok(entry) = entry {
                if let Some(file_name) = entry.path().file_stem() {
                    if let Some(name) = file_name.to_str() {
                        lists.push(name.to_string());
                    }
                }
            }
        }
        if lists.is_empty() {
            lists.push("default".to_string());
        }
        Ok(lists)
    }

    fn prompt_for_list(&self) -> io::Result<Option<String>> {
        let lists = self.get_available_lists()?;
        
        println!("\nAvailable lists:");
        for (i, list) in lists.iter().enumerate() {
            println!("{}. {}", i + 1, list);
        }
        println!("\nEnter list number or name (press Enter for 'default', !q to cancel):");
        
        let mut input = String::new();
        stdout().flush()?;
        stdin().read_line(&mut input)?;
        
        let input = input.trim();
        
        if input == "!q" {
            return Ok(None);
        }
        
        if input.is_empty() {
            return Ok(Some("default".to_string()));
        }
        
        // Try to parse as number first
        if let Ok(num) = input.parse::<usize>() {
            if num > 0 && num <= lists.len() {
                return Ok(Some(lists[num - 1].clone()));
            }
        }
        
        // If not a number, use as list name
        Ok(Some(input.to_string()))
    }

    fn list_all_tasks(&self) -> io::Result<()> {
        let lists = self.get_available_lists()?;
        
        if lists.is_empty() {
            println!("No todo lists found.");
            return Ok(());
        }

        println!("\n=== All Todo Lists ===");
        for list_name in lists {
            println!("\n📋 {}", list_name);
            println!("-------------------");
            
            let file_path = self.get_list_path(&list_name);
            if !file_path.exists() {
                println!("  (empty)");
                continue;
            }

            let file = File::open(file_path)?;
            let reader = BufReader::new(file);

            let mut has_tasks = false;
            for (index, line) in reader.lines().enumerate() {
                if let Ok(task) = line {
                    has_tasks = true;
                    println!("  {}. {}", index + 1, task);
                }
            }
            
            if !has_tasks {
                println!("  (empty)");
            }
        }
        Ok(())
    }

    fn copy_to_clipboard(text: &str) -> io::Result<bool> {
        // Try xsel first
        let xsel_result = Command::new("xsel")
            .arg("--clipboard")
            .arg("--input")
            .stdin(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                if let Some(stdin) = child.stdin.as_mut() {
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait().map(|_| true)
            });

        if xsel_result.is_ok() {
            return Ok(true);
        }

        // Fallback to xclip
        let xclip_result = Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .stdin(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                if let Some(stdin) = child.stdin.as_mut() {
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait().map(|_| true)
            });

        Ok(xclip_result.is_ok())
    }

    fn use_tag(&self, task_num: usize, list_name: &str, tag_num: Option<usize>) -> io::Result<()> {
        let list_path = self.get_list_path(list_name);
        if !list_path.exists() {
            println!("List '{}' not found.", list_name);
            return Ok(());
        }

        // Clone items at the start so we can modify them later if needed
        let mut items: Vec<TodoItem> = BufReader::new(File::open(&list_path)?)
            .lines()
            .filter_map(Result::ok)
            .map(|line| TodoItem::from_string(&line))
            .collect();

        if task_num == 0 || task_num > items.len() {
            println!("Error: Invalid task number");
            return Ok(());
        }

        let item = &items[task_num - 1];
        
        if item.tags.is_empty() {
            println!("No tags found for task {}.", task_num);
            return Ok(());
        }

        // Verify path exists
        let selected_tag = match tag_num {
            Some(n) if n > 0 && n <= item.tags.len() => item.tags[n - 1].clone(),
            Some(_) => {
                println!("Invalid tag number. Available tags:");
                for (i, tag) in item.tags.iter().enumerate() {
                    println!("{}. {}", i + 1, tag.display());
                }
                return Ok(());
            }
            None if item.tags.len() == 1 => item.tags[0].clone(),
            None => {
                println!("Multiple tags available. Please specify tag number:");
                for (i, tag) in item.tags.iter().enumerate() {
                    println!("{}. {}", i + 1, tag.display());
                }
                return Ok(());
            }
        };

        let path = PathBuf::from(&selected_tag);
        
        // Check if path exists
        if !path.exists() {
            println!("Warning: Path does not exist: {}", path.display());
            println!("Would you like to remove this tag? (y/N)");
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            if input.trim().to_lowercase() == "y" {
                let mut items = items;
                items[task_num - 1].tags.retain(|t| t != &selected_tag);
                let mut file = File::create(&list_path)?;
                for item in items {
                    writeln!(file, "{}", item.to_string())?;
                }
                println!("Tag removed.");
            }
            return Ok(());
        }

        // Handle existing path
        if path.is_dir() {
            let cd_command = format!("cd {}", path.display());
            
            // If in eval mode, only output the command
            if std::env::args().any(|arg| arg == "--eval") {
                print!("{}", cd_command);
                return Ok(());
            }

            // Try to copy to clipboard
            let copied = Self::copy_to_clipboard(&cd_command)?;

            // Show instructions
            println!("\nTo change directory, either:");
            println!("1. Copy and paste this command{}:",
                if copied { " (already copied to clipboard)" } else { "" });
            println!("   {}", cd_command);
            println!("2. Or use: eval $(todo use --eval 1 1 in rust)");
            
        } else if path.is_file() {
            let file_path = path.display().to_string();
            let copied = Self::copy_to_clipboard(&file_path)?;

            println!("Selected path is a file: {}", file_path);
            if copied {
                println!("File path copied to clipboard!");
            }
        }

        Ok(())
    }

    fn cleanup_list(&self, list_name: &str) -> io::Result<()> {
        let list_path = self.get_list_path(list_name);
        if list_path.exists() {
            fs::remove_file(&list_path)?;
            println!("List '{}' has been reset.", list_name);
        }
        Ok(())
    }
}

fn print_usage() {
    println!("Usage:");
    println!("  todo add <task> to <list>     - Add a task to a specific list");
    println!("  todo list                     - Show all available lists");
    println!("  todo list all                 - Show all lists and their tasks");
    println!("  todo list <list>              - List all tasks in a specific list");
    println!("  todo remove <num> from <list> - Remove task by number from a list");
    println!("  todo edit <num> in <list> <new_text> - Edit a task in a list");
    println!("  todo tag <num> in <list>            - Tag current directory to task");
    println!("  todo tag <file> <num> in <list>     - Tag specific file to task");
    println!("  todo use <num> in <list>           - Use first/only tag of task");
    println!("  todo use <num> <tag_num> in <list> - Use specific tag of task");
    println!("  todo cleanup <list>               - Reset a specific list");
}

fn main() -> io::Result<()> {
    // Set backtrace at start of program
    std::env::set_var("RUST_BACKTRACE", "1");
    
    let app = TodoApp::new()?;
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "add" => {
            if args.len() < 3 {
                println!("Usage: todo add <task> [to <list>]");
                return Ok(());
            }
            
            let has_list = args.windows(2).any(|w| w[0] == "to");
            
            if has_list {
                // Original behavior for "todo add <task> to <list>"
                if args.len() < 5 || args[args.len()-2] != "to" {
                    println!("Usage: todo add <task> to <list>");
                    return Ok(());
                }
                let list_name = &args[args.len()-1];
                let task = args[2..args.len()-2].join(" ");
                app.add_task(&task, list_name)?;
            } else {
                // New interactive behavior when no list is specified
                let task = args[2..].join(" ");
                
                match app.prompt_for_list()? {
                    Some(list_name) => {
                        app.add_task(&task, &list_name)?;
                    }
                    None => {
                        println!("Operation cancelled");
                    }
                }
            }
        }
        "list" => {
            match args.get(2).map(|s| s.as_str()) {
                Some("all") => app.list_all_tasks()?,
                Some(list_name) => app.list_tasks(list_name)?,
                None => app.list_all_lists()?,
            }
        }
        "remove" => {
            if args.len() < 4 || args[args.len()-2] != "from" {
                println!("Usage: todo remove <num> from <list>");
                return Ok(());
            }
            let list_name = &args[args.len()-1];
            if let Ok(num) = args[2].parse::<usize>() {
                app.remove_task(num, list_name)?;
            } else {
                println!("Error: Invalid task number");
            }
        }
        "edit" => {
            if args.len() < 6 || args[3] != "in" {
                println!("Usage: todo edit <num> in <list> <new_text>");
                return Ok(());
            }
            if let Ok(num) = args[2].parse::<usize>() {
                let list_name = &args[4];
                let new_text = args[5..].join(" ");
                app.edit_task(num, &new_text, list_name)?;
            } else {
                println!("Error: Invalid task number");
            }
        }
        "tag" => {
            if args.len() >= 5 && args[args.len()-2] == "in" {
                let list_name = &args[args.len()-1];
                if let Ok(num) = args[args.len()-3].parse::<usize>() {
                    // Get the file argument if it exists
                    let file_arg = if args.len() > 5 {
                        Some(args[2].as_str())
                    } else {
                        None
                    };
                    println!("Passing file argument: {:?}", file_arg);  // Debug print
                    app.add_tag(file_arg, num, list_name)?;
                } else {
                    println!("Error: Invalid task number");
                }
            } else {
                println!("Usage: todo tag <file> <num> in <list>");
            }
        }
        "use" => {
            if args.len() < 4 || args[args.len()-2] != "in" {
                println!("Usage: todo use <num> [tag_num] in <list>");
                return Ok(());
            }
            let list_name = &args[args.len()-1];
            let task_num = args[2].parse::<usize>().unwrap_or(0);
            let tag_num = if args.len() > 5 {
                args[3].parse::<usize>().ok()
            } else {
                None
            };
            app.use_tag(task_num, list_name, tag_num)?;
        }
        "cleanup" => {
            if args.len() < 3 {
                println!("Usage: todo cleanup <list>");
                return Ok(());
            }
            app.cleanup_list(&args[2])?;
        }
        _ => print_usage(),
    }
    Ok(())
}
