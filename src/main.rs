use anyhow::anyhow;
use anyhow::Result;

use clap::{AppSettings, Clap};
use colored::Colorize;
use mime::Mime;
use reqwest::header;
use reqwest::Client;
use reqwest::Response;
use reqwest::Url;
use std::collections::HashMap;
use std::str::FromStr;

// 以下部分用于处理CLI

// 定义HTTPie的CLI主入口，包含若干子命令
/// A naive httpie implementation with Rust, can you imagine how easy it is?
#[derive(Clap, Debug)]
#[clap(version = "1.0", author = "Fenix")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

// 子命令分别对应不同的HTTP方法，目前只支持get / post
#[derive(Clap, Debug)]
enum SubCommand {
    Get(Get),
    Post(Post),
}

// get子命令

/// feed get with an url and we will retrieve the response for you
#[derive(Clap, Debug)]
struct Get {
    // http 请求的url
    #[clap(parse(try_from_str = parse_url))]
    url: String,
}

// post 子命令。 需要输入一个url，和若干个可选的key=value，用于提供json body

/// feed post with an url and optional key=value pairs. We will post the data/// as JSON, and retrieve the response for you
#[derive(Clap, Debug)]
struct Post {
    // http请求的url
    #[clap(parse(try_from_str=parse_url))]
    url: String,
    // http请求的body
    #[clap(parse(try_from_str=parse_kv_pair))]
    body: Vec<KvPair>,
}

/// 命令行中的 key=value 可以通过 parse_kv_pair 解析成 KvPair 结构
#[derive(Debug)]
struct KvPair {
    k: String,
    v: String,
}

/// 当我们实现 FromStr trait 后，可以用 str.parse() 方法将字符串解析成 KvPair
impl FromStr for KvPair {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 使用 = 进行split，这个会得到一个迭代器
        let mut split = s.split("=");
        let err = || anyhow!(format!("Failed to parse {}", s));
        Ok(Self {
            // 迭代器中获取的第一个结果未key，迭代器返回Some(T)/None
            // 我们将其转换为Ok(T)/Err(E)，然后用？处理错误
            k: (split.next().ok_or_else(err)?).to_string(),
            // 从迭代器中取第二个结果作为 value
            v: (split.next().ok_or_else(err)?).to_string(),
        })
    }
}

/// 因为我们为 KvPair 实现了 FromStr，这里可以直接 s.parse() 得到 KvPair
fn parse_kv_pair(s: &str) -> Result<KvPair> {
    Ok(s.parse()?)
}

fn parse_url(s: &str) -> Result<String> {
    // 这里我们仅仅检查一下 URL 是否合法
    let _url: Url = s.parse()?;

    Ok(s.into())
}

/// 处理 get 子命令
async fn get(client: Client, args: &Get) -> Result<()> {
    let resp = client.get(&args.url).send().await?;
    Ok(print_resp(resp).await?)
}

/// 处理 post 子命令
async fn post(client: Client, args: &Post) -> Result<()> {
    let mut body = HashMap::new();
    for pair in args.body.iter() {
        body.insert(&pair.k, &pair.v);
    }

    let resp = client.post(&args.url).json(&body).send().await?;
    Ok(print_resp(resp).await?)
}

// 打印服务器版本号 + 状态码
fn print_status(resp: &Response) {
    let status = format!("{:?} {}", resp.version(), resp.status()).blue();
    println!("{}\n", status)
}

// 打印服务器返回的HTTP header
fn print_headers(resp: &Response) {
    for (name, value) in resp.headers() {
        println!("{}: {:?}", name.to_string().green(), value);
    }

    print!("\n")
}

// 打印服务器返回的HTTP body
fn print_body(m: Option<Mime>, body: &String) {
    match m {
        Some(v) if v == mime::APPLICATION_JSON => {
            println!("{}", jsonxf::pretty_print(body).unwrap().cyan())
        }

        // 其他的mime
        _ => println!("{}", body),
    }
}

/// 将服务器返回的content-type 解析成mime类型
fn get_content_type(resp: &Response) -> Option<Mime> {
    resp.headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().parse().unwrap())
}

/// 打印整个响应
async fn print_resp(resp: Response) -> Result<()> {
    print_status(&resp);
    print_headers(&resp);
    let mime = get_content_type(&resp);
    let body = resp.text().await?;
    print_body(mime, &body);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    let mut headers = header::HeaderMap::new();

    // 为header添加一个缺省的头
    headers.insert("X-POWERED-BY", "Rust".parse()?);
    headers.insert(header::USER_AGENT, "Rust Httpie".parse()?);

    // 生成一个http客户端
    let cli = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    let result = match opts.subcmd {
        SubCommand::Get(ref args) => get(cli, args).await?,
        SubCommand::Post(ref args) => post(cli, args).await?,
    };

    Ok(result)
}
