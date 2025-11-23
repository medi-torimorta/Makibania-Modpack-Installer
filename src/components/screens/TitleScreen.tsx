import DescriptionIcon from "@mui/icons-material/Description";
import {
  Backdrop,
  Box,
  Button,
  CircularProgress,
  Container,
  IconButton,
  Stack,
  Typography,
  useTheme,
} from "@mui/material";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { Translation } from "../../utils/localizer";
import { InstallerMode, installerModes } from "./InstallerScreen";

export type TitleStatus = {
  canInstall: boolean;
  canUpdate: boolean;
};

type TitleScreenProps = {
  onModeSelect: (mode: InstallerMode) => void;
  translation: Translation;
};

export default function TitleScreen(props: TitleScreenProps) {
  const [initializedStatus, setInitialized] = useState<TitleStatus | null>(
    null
  );

  const isInitialized = useRef(false);

  const theme = useTheme();

  const buttonWidth = 36;
  const initialize = async () => {
    const status = await invoke<TitleStatus>("initialize_title");
    setInitialized(status);
  };

  useEffect(() => {
    if (isInitialized.current) {
      return;
    }
    isInitialized.current = true;
    initialize();
  }, []);

  return (
    <Container
      maxWidth="md"
      sx={{
        height: "100%",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {initializedStatus && (
        <Stack spacing={3} alignItems="center">
          <Typography
            variant="h3"
            component="h1"
            fontWeight={700}
            textAlign="center"
            sx={{
              background: `linear-gradient(45deg, ${theme.palette.primary.dark} 30%, ${theme.palette.primary.light} 60%)`,
              backgroundClip: "text",
              WebkitBackgroundClip: "text",
              WebkitTextFillColor: "transparent",
              letterSpacing: "-0.02em",
            }}
          >
            {props.translation.appTitle}
          </Typography>
          <Typography
            color="text.secondary"
            textAlign="center"
            variant="subtitle1"
          >
            {props.translation.titleMessage}
          </Typography>
          <Stack
            direction="row"
            spacing={2}
            alignItems="center"
            justifyContent="center"
          >
            <Box sx={{ width: buttonWidth }} />
            <Stack spacing={2} sx={{ width: "200px" }}>
              {installerModes.map((mode) => (
                <Button
                  key={`title-${mode}`}
                  disabled={
                    mode === "install"
                      ? !initializedStatus.canInstall
                      : !initializedStatus.canUpdate
                  }
                  variant="contained"
                  onClick={() => props.onModeSelect(mode)}
                  sx={{ height: buttonWidth }}
                >
                  {mode === "install"
                    ? props.translation.install
                    : props.translation.update}
                </Button>
              ))}
            </Stack>
            <IconButton
              color="primary"
              onClick={() => invoke("open_log_folder")}
              sx={{
                width: buttonWidth,
                height: buttonWidth,
                border: 1,
                borderRadius: 1,
                boxShadow: 1,
              }}
            >
              <DescriptionIcon />
            </IconButton>
          </Stack>
        </Stack>
      )}
      <Backdrop
        sx={(theme) => ({ zIndex: theme.zIndex.drawer + 1 })}
        open={!initializedStatus}
      >
        <CircularProgress color="inherit" />
      </Backdrop>
    </Container>
  );
}
