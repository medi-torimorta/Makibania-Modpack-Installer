import {
  Backdrop,
  Box,
  Button,
  CircularProgress,
  CssBaseline,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
} from "@mui/material";

import { createTheme, ThemeProvider } from "@mui/material/styles";
import { invoke } from "@tauri-apps/api/core";
import { exit } from "@tauri-apps/plugin-process";
import { useEffect, useRef, useState } from "react";
import "./App.css";
import TitleBar from "./components/TitleBar";
import InstallerScreen, {
  InstallerMode,
} from "./components/screens/InstallerScreen";
import TitleScreen from "./components/screens/TitleScreen";
import { Language, translations } from "./utils/localizer";

export type Screen = "title" | "installer";

type TitleStatus = {
  isExistsConfig: boolean;
};

type ModeResult = {
  isAccept: boolean;
  error?: string;
};

export default function App() {
  const theme = createTheme({
    palette: {
      primary: {
        main: "#d35555",
      },
      secondary: {
        main: "#6fb1af",
      },
    },
  });

  const [initializedStatus, setInitialized] = useState<TitleStatus | null>(
    null
  );
  const [language, setLanguage] = useState<Language>("ja");
  const [screen, setScreen] = useState<Screen>("title");
  const [installerMode, setInstallerMode] = useState<InstallerMode | null>(
    null
  );
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const isInitialized = useRef(false);

  useEffect(() => {
    if (isInitialized.current) {
      return;
    }
    isInitialized.current = true;
    const initialize = async () => {
      const status = await invoke<TitleStatus>("initialize_title");
      setInitialized(status);
    };
    initialize();
  }, []);

  const translation = translations[language];
  const titleLabel = screen === "title" ? "" : translation.installerTitle;

  const getScreen = () => {
    switch (screen) {
      case "title":
        return (
          <TitleScreen
            onModeSelect={async (mode) => {
              const result = await invoke<ModeResult>("select_mode", { mode });
              if (!result.isAccept) {
                setErrorMessage(
                  translation.occurredError +
                    (result.error || "<unknown error>")
                );
                return;
              }
              setInstallerMode(mode);
              setScreen("installer");
            }}
            translation={translation}
          />
        );
      case "installer":
        return (
          <InstallerScreen
            mode={installerMode!}
            onComplete={async () => {
              await exit();
            }}
            onDismissError={() => {
              setInstallerMode(null);
              setScreen("title");
            }}
            translation={translation}
          />
        );
    }
  };

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <Box sx={{ display: "flex", flexDirection: "column", height: "100vh" }}>
        <TitleBar
          title={titleLabel}
          language={language}
          onLanguageChange={setLanguage}
          translation={translation}
        />
        <Box sx={{ flexGrow: 1, overflow: "auto" }}>{getScreen()}</Box>
      </Box>
      <Dialog
        open={errorMessage != null}
        aria-describedby="description"
        onClose={() => setErrorMessage(null)}
        sx={{ whiteSpace: "pre-line" }}
      >
        <DialogContent>
          <DialogContentText id="description">{errorMessage}</DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setErrorMessage(null)} autoFocus>
            {translation.close}
          </Button>
        </DialogActions>
      </Dialog>
      <Backdrop
        sx={(theme) => ({ zIndex: theme.zIndex.drawer + 1 })}
        open={!initializedStatus}
      >
        <CircularProgress color="inherit" />
      </Backdrop>
    </ThemeProvider>
  );
}
