import React, { useState } from 'react';
import { SendFlow } from './components/SendFlow';
import { ReceiveFlow } from './components/ReceiveFlow';
import './App.css';

function App() {
  const [activeTab, setActiveTab] = useState<'send' | 'receive'>('send');

  return (
    <div className="app">
      <header className="app-header">
        <h1>Arkade Wallet</h1>
        <div className="tab-bar">
          <button
            className={activeTab === 'send' ? 'active' : ''}
            onClick={() => setActiveTab('send')}
          >
            ⚡ Send
          </button>
          <button
            className={activeTab === 'receive' ? 'active' : ''}
            onClick={() => setActiveTab('receive')}
          >
            📥 Receive
          </button>
        </div>
      </header>

      <main className="app-main">
        {activeTab === 'send' && <SendFlow />}
        {activeTab === 'receive' && <ReceiveFlow />}
      </main>

      <footer className="app-footer">
        <p>Powered by <a href="https://github.com/Truja503/satspath" target="_blank" rel="noopener">SatsPath</a> + <a href="https://github.com/arkade-os" target="_blank" rel="noopener">Arkade</a></p>
      </footer>
    </div>
  );
}

export default App;