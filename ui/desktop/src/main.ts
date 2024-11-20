import 'dotenv/config';
import { app, BrowserWindow } from 'electron';
import path from 'node:path';
import { spawn } from 'child_process';
import started from "electron-squirrel-startup";

// Handle creating/removing shortcuts on Windows when installing/uninstalling.
if (started) app.quit();

// Start the goosed binary
const startGoosed = () => {
  // In development, the binary is in src/bin
  // In production, it will be in the resources/bin directory
  const isDev = process.env.NODE_ENV === 'development';
  let goosedPath;
  
  if (isDev) {
    // In development, use the absolute path from the project root
    goosedPath = path.join(process.cwd(), 'src', 'bin', process.platform === 'win32' ? 'goosed.exe' : 'goosed');
  } else {
    // In production, use the path relative to the app resources
    goosedPath = path.join(process.resourcesPath, 'bin', process.platform === 'win32' ? 'goosed.exe' : 'goosed');
  }

  console.log(`Starting goosed from: ${goosedPath}`);
  
  const goosedProcess = spawn(goosedPath);

  goosedProcess.stdout.on('data', (data) => {
    console.log(`goosed stdout: ${data}`);
  });

  goosedProcess.stderr.on('data', (data) => {
    console.error(`goosed stderr: ${data}`);
  });

  goosedProcess.on('close', (code) => {
    console.log(`goosed process exited with code ${code}`);
  });

  goosedProcess.on('error', (err) => {
    console.error('Failed to start goosed:', err);
  });

  // Ensure goosed is terminated when the app quits
  app.on('will-quit', () => {
    goosedProcess.kill();
  });
};

const createWindow = () => {
  const mainWindow = new BrowserWindow({
    titleBarStyle: 'hidden',
    trafficLightPosition: { x: 16, y: 18 },
    width: 1024,
    height: 768,
    icon: path.join(__dirname, '../images/icon.png'),
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
    },
  });

  // and load the index.html of the app.
  if (MAIN_WINDOW_VITE_DEV_SERVER_URL) {
    mainWindow.loadURL(MAIN_WINDOW_VITE_DEV_SERVER_URL);
  } else {
    mainWindow.loadFile(path.join(__dirname, `../renderer/${MAIN_WINDOW_VITE_NAME}/index.html`));
  }

  console.log(MAIN_WINDOW_VITE_NAME);

  // Open the DevTools.
  mainWindow.webContents.openDevTools();
};

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
app.whenReady().then(() => {
  // Get the server startup configuration
  const shouldStartServer = (import.meta.env.VITE_START_EMBEDDED_SERVER || 'yes').toLowerCase() === 'yes';
  
  if (shouldStartServer) {
    console.log('Starting embedded goosed server');
    startGoosed();
  } else {
    console.log('Skipping embedded server startup (disabled by configuration)');
  }
  createWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

// Quit when all windows are closed, except on macOS.
app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});