const http = require("http");
const cp = require("child_process");

async function test(){
        const srv = http.createServer((req, res) => {
                res.writeHead(200, { "Content-Type": "text/plain" });
                res.socket.resetAndDestroy(); // RST
        });
        srv.listen(3000);
        const promises = [];
        const curl = cp.exec("curl -v -x socks5h://0.0.0.0:18080 http://localhost:3000/");
        curl.stdout.pipe(process.stdout);
        curl.stderr.pipe(process.stderr);
        await new Promise((resolve, reject) => {
            curl.on("exit", resolve);
        });
        srv.close();
}

test().catch(console.error);
