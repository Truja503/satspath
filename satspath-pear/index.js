const Hyperswarm = require('hyperswarm');
const crypto = require('crypto');
const fs = require('fs');

const command = process.argv[2];
const alias = process.argv[3];

if (!command || !alias) {
  console.error("Usage: node index.js <resolve|seed> <alias> [profile.json]");
  process.exit(1);
}

function getTopic(aliasStr) {
  // Hash the alias to preserve privacy (P2P-03)
  return crypto.createHash('sha256').update(aliasStr.toLowerCase().trim()).digest();
}

async function main() {
  const swarm = new Hyperswarm();
  const topic = getTopic(alias);

  if (command === 'seed') {
    const profilePath = process.argv[4];
    if (!profilePath) {
      console.error("Usage for seed: node index.js seed <alias> <profile.json>");
      process.exit(1);
    }
    const profileData = fs.readFileSync(profilePath, 'utf8');

    swarm.on('connection', (conn, info) => {
      // When a peer connects, send them the profile
      conn.write(profileData);
    });

    swarm.join(topic, { server: true, client: false });
    await swarm.flush();
    console.log(`Seeding profile for ${alias} on topic ${topic.toString('hex')}...`);
    
    // Keep alive
    process.on('SIGINT', () => {
      swarm.destroy();
      process.exit(0);
    });
  } 
  else if (command === 'resolve') {
    let resolved = false;

    // Timeout after 10 seconds
    const timeout = setTimeout(() => {
      if (!resolved) {
        console.error("Resolution timeout");
        swarm.destroy();
        process.exit(1);
      }
    }, 10000);

    swarm.on('connection', (conn, info) => {
      if (resolved) return;
      
      let dataChunks = [];
      conn.on('data', (data) => {
        dataChunks.push(data);
      });

      conn.on('end', () => {
        if (resolved) return;
        try {
          const fullData = Buffer.concat(dataChunks).toString('utf8');
          // Parse just to ensure it's JSON, we let Rust do the real validation
          JSON.parse(fullData);
          
          // Print strictly the JSON to stdout so Rust can parse it
          console.log(fullData);
          resolved = true;
          clearTimeout(timeout);
          swarm.destroy();
          process.exit(0);
        } catch (err) {
          // Ignore bad data from peers
        }
      });
      
      // Some peers might just send and wait, let's process after a short delay if no 'end' is emitted
      setTimeout(() => {
        if (!resolved && dataChunks.length > 0) {
           try {
             const fullData = Buffer.concat(dataChunks).toString('utf8');
             JSON.parse(fullData);
             console.log(fullData);
             resolved = true;
             clearTimeout(timeout);
             swarm.destroy();
             process.exit(0);
           } catch (e) {}
        }
      }, 1000);
    });

    swarm.join(topic, { server: false, client: true });
    await swarm.flush();
  }
}

main().catch(err => {
  console.error("Error:", err);
  process.exit(1);
});
